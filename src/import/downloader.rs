use futures_util::StreamExt;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Client, Response, Url};
use std::fs::File;
use std::io::{self, copy};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Represents the progress of a download
#[derive(Debug, Clone)]
pub struct Progress {
    /// Total size of the download in bytes (if known)
    pub total_size: Option<u64>,
    /// Number of bytes downloaded so far
    pub downloaded: u64,
    /// Whether the download is complete
    pub complete: bool,
}

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

        // Create output file
        let mut file = File::create(&final_path)?;

        // Stream the response to file
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        // Send initial progress update
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded: 0,
                complete: false,
            })
            .await;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let chunk_size = chunk.len() as u64;
                    copy(&mut chunk.as_ref(), &mut file)?;

                    // Update download progress
                    downloaded += chunk_size;

                    // Send progress update
                    let _ = progress_tx
                        .send(Progress {
                            total_size,
                            downloaded,
                            complete: false,
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
                complete: true,
            })
            .await;

        // Return the actual path used for the download
        Ok(final_path)
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
}

/// Parse filename from Content-Disposition header
/// Returns Some(filename) if successful, None otherwise
fn parse_content_disposition(content_disposition: &str) -> Option<String> {
    // Look for filename="..." or filename*=... patterns
    if let Some(pos) = content_disposition.find("filename=\"") {
        let start = pos + "filename=\"".len();
        if let Some(end) = content_disposition[start..].find('"') {
            return Some(content_disposition[start..(start + end)].to_string());
        }
    }

    // Look for filename=... (without quotes)
    if let Some(pos) = content_disposition.find("filename=") {
        let start = pos + "filename=".len();
        let end = content_disposition[start..]
            .find(|c: char| c.is_whitespace() || c == ';')
            .unwrap_or(content_disposition[start..].len());
        if end > 0 {
            return Some(content_disposition[start..(start + end)].to_string());
        }
    }

    None
}
