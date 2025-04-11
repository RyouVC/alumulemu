//! HTTP download implementation
//!
//! This module provides functionality for downloading files from HTTP sources,
//! handling redirects, retries, and progress tracking.

use futures_util::StreamExt;
use reqwest::{
    Client, Response, Url,
    header::{self, HeaderMap, HeaderValue},
};
use std::{
    io,
    path::{Path, PathBuf},
};
use tokio::{fs::File, io::AsyncWriteExt, sync::mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{Level, debug, error, info, instrument, span, trace};

use super::models::{DownloadStatus, PartialDownloadError, Progress, parse_content_disposition};

pub struct Downloader {
    client: Client,
    max_redirects: usize,
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

impl Downloader {
    pub fn new() -> Self {
        // Setup headers exactly like curl
        let mut headers = HeaderMap::new();
        // headers.insert(header::USER_AGENT, HeaderValue::from_static("curl/8.9.1"));
        headers.insert(header::ACCEPT, HeaderValue::from_static("*/*"));

        // Create a client that doesn't follow redirects automatically
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .build()
            .unwrap();

        Self {
            client,
            max_redirects: 10,
        }
    }

    pub fn with_max_redirects(mut self, max_redirects: usize) -> Self {
        self.max_redirects = max_redirects;
        self
    }

    pub async fn download_file<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
    ) -> io::Result<PathBuf> {
        // Create a null channel that drops all progress updates
        let (tx, _) = mpsc::channel(10);
        self.download_file_with_progress(url, output_path, tx).await
    }

    pub async fn get_with_redirects(&self, url: &str) -> io::Result<Response> {
        let mut current_url = url.to_string();
        let mut redirect_count = 0;

        loop {
            // Send request
            let response = match self.client.get(&current_url).send().await {
                Ok(resp) => resp,
                Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
            };

            // If not a redirect or we've hit the max, return this response
            if !response.status().is_redirection() || redirect_count >= self.max_redirects {
                return Ok(response);
            }

            // Extract location header for the redirect
            let location = match response.headers().get(header::LOCATION) {
                Some(loc) => {
                    let loc_str = loc
                        .to_str()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                    // Handle relative URLs
                    let base_url = Url::parse(&current_url)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

                    base_url
                        .join(loc_str)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
                        .to_string()
                }
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Redirect without Location header",
                    ));
                }
            };

            // Update for next iteration
            current_url = location;
            redirect_count += 1;
        }
    }

    #[allow(dead_code)]
    pub async fn download_file_with_progress<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        progress_tx: mpsc::Sender<Progress>,
    ) -> io::Result<PathBuf> {
        let response = self.get_with_redirects(url).await?;

        // Check if output_path is a directory
        let output_path_ref = output_path.as_ref();
        let final_path = if output_path_ref.is_dir() {
            // Try to extract filename from Content-Disposition header
            let filename = if let Some(content_disposition) =
                response.headers().get(header::CONTENT_DISPOSITION)
            {
                println!("Content-Disposition: {:?}", content_disposition);

                let content_disposition_str = content_disposition
                    .to_str()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                // Parse Content-Disposition for filename
                // Example: attachment; filename="filename.zip"
                parse_content_disposition(content_disposition_str)
            } else {
                None
            };

            // If we couldn't get filename from Content-Disposition, try to get it from the URL
            let filename = filename
                .or_else(|| {
                    let binding = Url::parse(url).ok()?;
                    let url_path = binding.path();
                    let path = Path::new(url_path);
                    path.file_name()?.to_str().map(|s| s.to_string())
                })
                .unwrap_or_else(|| {
                    // If all else fails, use a generic filename with timestamp
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    format!("download_{}.bin", now)
                });

            output_path_ref.join(filename)
        } else {
            output_path_ref.to_path_buf()
        };

        // Get content length if available
        let total_size = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|cl| cl.to_str().ok())
            .and_then(|cl| cl.parse::<u64>().ok());

        // Create output file using tokio's async file operations
        let mut file = File::create(&final_path).await?;

        // Stream the response to file
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        // Send initial progress update
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded: 0,
                status: DownloadStatus::Downloading,
                file_path: Some(final_path.clone()),
            })
            .await;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let chunk_size = chunk.len() as u64;
                    file.write_all(&chunk).await?;

                    // Update download progress
                    downloaded += chunk_size;

                    // Send progress update
                    let _ = progress_tx
                        .send(Progress {
                            total_size,
                            downloaded,
                            status: DownloadStatus::Downloading,
                            file_path: Some(final_path.clone()),
                        })
                        .await;
                }
                Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
            }
        }

        // Send final progress update
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded,
                status: DownloadStatus::Completed,
                file_path: Some(final_path.clone()),
            })
            .await;

        // Return the actual path used for the download
        Ok(final_path)
    }

    #[instrument(name = "download_file", level = "debug", skip(self, progress_tx, cancel_token, output_path), fields(url = %url))]
    pub async fn download_file_with_progress_cancellable<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        progress_tx: mpsc::Sender<Progress>,
        cancel_token: CancellationToken,
    ) -> io::Result<PathBuf> {
        trace!("Starting download with progress tracking");

        const MAX_RETRIES: usize = 3;
        let mut retry_count = 0;
        let mut last_error: Option<io::Error> = None;
        let mut downloaded_so_far: u64 = 0;

        // Keep trying until we succeed or exceed max retries
        while retry_count <= MAX_RETRIES {
            if retry_count > 0 {
                info!(attempt = retry_count + 1, "Retrying download");

                // Check if cancelled during retry wait
                if cancel_token.is_cancelled() {
                    info!("Download cancelled during retry wait");
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "Download cancelled",
                    ));
                }

                // Exponential backoff delay
                let delay = std::time::Duration::from_secs(2u64.pow(retry_count as u32));
                info!(
                    retry_count = retry_count,
                    delay_secs = delay.as_secs(),
                    "Waiting before retry"
                );
                tokio::time::sleep(delay).await;
            }

            // Attempt the download
            match self
                .download_with_retry_internal(
                    url,
                    output_path.as_ref(),
                    progress_tx.clone(),
                    cancel_token.clone(),
                    downloaded_so_far,
                )
                .await
            {
                Ok(path) => return Ok(path),
                Err(e) => {
                    // If the error is from cancellation, don't retry
                    if e.kind() == io::ErrorKind::Interrupted {
                        return Err(e);
                    }

                    // Extract downloaded bytes from the error if it's our custom error type
                    if let Some(partial_err) = e
                        .get_ref()
                        .and_then(|err_ref| err_ref.downcast_ref::<PartialDownloadError>())
                    {
                        // Update our tracking with actual bytes downloaded in the failed attempt
                        let partial_bytes = partial_err.bytes_downloaded;
                        if partial_bytes > downloaded_so_far {
                            info!(
                                previous = downloaded_so_far,
                                current = partial_bytes,
                                "Updating download progress after partial failure"
                            );
                            downloaded_so_far = partial_bytes;
                        }
                    }

                    // Determine if error is retryable (connection issues, timeouts, etc.)
                    let is_retryable = match e.kind() {
                        io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock => true,
                        _ => {
                            e.to_string().contains("network")
                                || e.to_string().contains("connection")
                                || e.to_string().contains("timeout")
                        }
                    };

                    if is_retryable && retry_count < MAX_RETRIES {
                        error!(
                            error = %e,
                            retry = retry_count + 1,
                            max_retries = MAX_RETRIES,
                            downloaded_bytes = downloaded_so_far,
                            "Download failed with retryable error"
                        );

                        // Update for next retry
                        retry_count += 1;
                        last_error = Some(e);
                    } else {
                        // Non-retryable error or max retries exceeded
                        error!(
                            error = %e,
                            retry_count = retry_count,
                            "Download failed permanently"
                        );
                        return Err(e);
                    }
                }
            }
        }

        // If we got here, we've exceeded retries
        Err(last_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Download failed after {} retries", MAX_RETRIES),
            )
        }))
    }

    // Internal function that does the actual download work, can be retried
    async fn download_with_retry_internal(
        &self,
        url: &str,
        output_path: &Path,
        progress_tx: mpsc::Sender<Progress>,
        cancel_token: CancellationToken,
        resume_from: u64,
    ) -> io::Result<PathBuf> {
        // Build the request with range header if resuming
        let mut request_builder = self.client.get(url);
        if resume_from > 0 {
            request_builder =
                request_builder.header(header::RANGE, format!("bytes={}-", resume_from));
            info!(resume_from = resume_from, "Attempting to resume download");
        }

        // Send the request
        let response = match request_builder.send().await {
            Ok(resp) => resp,
            Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
        };

        // Handle redirects manually since we're not using automatic redirect following
        if response.status().is_redirection() {
            trace!("Following redirect manually");
            let response = self.get_with_redirects(url).await?;
            return self
                .process_response(
                    response,
                    url,
                    output_path,
                    progress_tx,
                    cancel_token,
                    resume_from,
                )
                .await;
        }

        // Handle the response
        self.process_response(
            response,
            url,
            output_path,
            progress_tx,
            cancel_token,
            resume_from,
        )
        .await
    }

    // Process the HTTP response and download the file
    async fn process_response(
        &self,
        response: Response,
        url: &str,
        output_path: &Path,
        progress_tx: mpsc::Sender<Progress>,
        cancel_token: CancellationToken,
        resume_from: u64,
    ) -> io::Result<PathBuf> {
        trace!(status = %response.status(), "Got response");

        // Check for errors first
        if !response.status().is_success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "HTTP error: {} {}",
                    response.status().as_u16(),
                    response.status().as_str()
                ),
            ));
        }

        // Check if already canceled
        if cancel_token.is_cancelled() {
            info!("Download cancelled before starting");
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Download cancelled",
            ));
        }

        // Check if output_path is a directory
        let final_path = if output_path.is_dir() {
            // Try to extract filename from Content-Disposition header
            let filename = if let Some(content_disposition) =
                response.headers().get(header::CONTENT_DISPOSITION)
            {
                trace!(content_disposition = ?content_disposition, "Content-Disposition header found");

                let content_disposition_str = content_disposition.to_str().map_err(|e| {
                    error!(error = %e, "Failed to convert Content-Disposition to string");
                    io::Error::new(io::ErrorKind::InvalidData, e)
                })?;

                // Parse Content-Disposition for filename
                let parsed_filename = parse_content_disposition(content_disposition_str);
                if let Some(ref name) = parsed_filename {
                    debug!(filename = %name, "Extracted filename from Content-Disposition");
                }
                parsed_filename
            } else {
                trace!("No Content-Disposition header found");
                None
            };

            // If we couldn't get filename from Content-Disposition, try to get it from the URL
            let filename = filename
                .or_else(|| {
                    trace!("Attempting to extract filename from URL");
                    let binding = Url::parse(url).ok()?;
                    let url_path = binding.path();
                    let path = Path::new(url_path);
                    let filename = path.file_name()?.to_str().map(|s| s.to_string());

                    if let Some(ref name) = filename {
                        debug!(filename = %name, "Extracted filename from URL path");
                    }

                    filename
                })
                .unwrap_or_else(|| {
                    // If all else fails, use a generic filename with timestamp
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let generic_name = format!("download_{}.bin", now);
                    debug!(filename = %generic_name, "Using generic filename");
                    generic_name
                });

            let final_path = output_path.join(&filename);
            debug!(path = ?final_path, "Final download path");
            final_path
        } else {
            debug!(path = ?output_path, "Using specified file path");
            output_path.to_path_buf()
        };

        // Get content length if available
        let total_size = match (
            response.headers().get(header::CONTENT_LENGTH),
            response.headers().get(header::CONTENT_RANGE),
        ) {
            // Content-Length header is present
            (Some(cl), _) => cl
                .to_str()
                .ok()
                .and_then(|cl| cl.parse::<u64>().ok())
                .map(|len| {
                    // If we're resuming, add the resumed amount
                    if resume_from > 0 {
                        len + resume_from
                    } else {
                        len
                    }
                }),

            // Content-Range header might have total size
            (_, Some(range)) => {
                // Parse Content-Range header (format: bytes 0-1234/5678)
                if let Ok(range_str) = range.to_str() {
                    let parts: Vec<&str> = range_str.split('/').collect();
                    if parts.len() == 2 {
                        parts[1].parse::<u64>().ok()
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            // No size info available
            _ => None,
        };

        if let Some(size) = total_size {
            info!(bytes = size, path = ?final_path, "Starting download");
        } else {
            info!(path = ?final_path, "Starting download of unknown size");
        }

        // Create or open output file
        let file = if resume_from > 0 && final_path.exists() {
            let mut options = tokio::fs::OpenOptions::new();
            options.write(true).append(true);
            options.open(&final_path).await?
        } else {
            // Start from beginning
            File::create(&final_path).await?
        };

        let mut file = file;
        let mut downloaded = resume_from;

        // Stream the response to file
        let mut stream = response.bytes_stream();

        // Send initial progress update with the file path
        trace!("Sending initial progress update");
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded,
                status: DownloadStatus::Downloading,
                file_path: Some(final_path.clone()),
            })
            .await;

        // Create a span for chunk processing
        let chunks_span = span!(Level::TRACE, "download_chunks", path = ?final_path);
        let _guard = chunks_span.enter();

        while let Some(chunk) = stream.next().await {
            // Check for cancellation
            if cancel_token.is_cancelled() {
                info!(
                    downloaded = downloaded,
                    "Download cancelled during progress"
                );
                // Close and delete the partial file
                let _ = file.shutdown().await;
                let _ = tokio::fs::remove_file(&final_path).await;
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Download cancelled",
                ));
            }

            match chunk {
                Ok(chunk) => {
                    let chunk_size = chunk.len() as u64;
                    file.write_all(&chunk).await?;

                    // Update download progress
                    downloaded += chunk_size;

                    // Log chunk at trace level
                    trace!(
                        bytes = downloaded,
                        chunk_size = chunk_size,
                        "Received chunk"
                    );

                    // Send progress update
                    let _ = progress_tx
                        .send(Progress {
                            total_size,
                            downloaded,
                            status: DownloadStatus::Downloading,
                            file_path: Some(final_path.clone()),
                        })
                        .await;
                }
                Err(e) => {
                    error!(error = %e, "Error downloading chunk");
                    // Create a partial download error that includes how many bytes were downloaded
                    let partial_err = io::Error::new(
                        io::ErrorKind::Other,
                        PartialDownloadError {
                            bytes_downloaded: downloaded,
                            source: io::Error::new(io::ErrorKind::Other, e),
                        },
                    );
                    return Err(partial_err);
                }
            }
        }

        // Make sure the file is completely written
        file.flush().await?;
        file.shutdown().await?;

        info!(bytes = downloaded, "Download completed");

        // Send final progress update
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded,
                status: DownloadStatus::Completed,
                file_path: Some(final_path.clone()),
            })
            .await;

        Ok(final_path)
    }
}
