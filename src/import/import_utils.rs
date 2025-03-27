use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::{error, info};

use crate::backend::admin::trigger_rescan;
use crate::import::IdImporter;
use crate::import::registry::{
    IdImportProvider, IdImportProviderObj, ImporterProvider, find_id_importer_for,
    get_importer_by_name,
};

/// Errors that can occur during the import process
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Importer not found: {0}")]
    ImporterNotFound(String),

    #[error("Invalid import source: {0}")]
    InvalidSource(String),

    #[error("Import failed: {0}")]
    ImportFailed(#[from] crate::import::ImportError),

    #[error("No suitable importer found for ID: {0}")]
    NoSuitableImporterForId(String),
}

impl IntoResponse for ImportError {
    fn into_response(self) -> Response {
        let error_msg = self.to_string();
        error!("Import error: {}", error_msg);

        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "error",
                "message": error_msg
            })),
        )
            .into_response()
    }
}

/// Unified importer interface result type
pub type ImportResult = std::result::Result<Response, ImportError>;

/// A helper struct that holds information needed for a generic import
pub struct ImportRequest {
    /// The name of the importer to use (optional if id is provided)
    pub importer_name: Option<String>,

    /// The ID to use for importing (for IdImporters)
    pub id: Option<String>,

    /// Path or URL to import from (for FileImporters)
    pub source: Option<String>,
}

impl ImportRequest {
    /// Create a new import request by ID
    pub fn new_id(id: impl Into<String>) -> Self {
        Self {
            importer_name: None,
            id: Some(id.into()),
            source: None,
        }
    }

    /// Create a new import request by ID with a specific importer
    pub fn new_id_with_importer(importer: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            importer_name: Some(importer.into()),
            id: Some(id.into()),
            source: None,
        }
    }

    /// Create a new import request from a source
    pub fn new_source(source: impl Into<String>) -> Self {
        Self {
            importer_name: None,
            id: None,
            source: Some(source.into()),
        }
    }

    /// Create a new import request from a source with a specific importer
    pub fn new_source_with_importer(
        importer: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            importer_name: Some(importer.into()),
            id: None,
            source: Some(source.into()),
        }
    }

    /// Convert a source string to a path if it's a local path
    fn source_to_path(&self) -> Option<PathBuf> {
        self.source.as_ref().and_then(|src| {
            // Simple check to see if this is a URL or a file path
            if src.starts_with("http://") || src.starts_with("https://") {
                None
            } else {
                Some(PathBuf::from_str(src).ok()?)
            }
        })
    }

    /// Perform the import operation
    pub async fn import(self) -> ImportResult {
        // Case 1: We have an ID and want to use an IdImporter
        if let Some(id) = self.id.clone() {
            return self.import_by_id(id).await;
        }

        // Case 2: We have a source and want to use a FileImporter
        if let Some(source) = self.source.clone() {
            return self.import_from_source(source).await;
        }

        // No valid import parameters
        Err(ImportError::InvalidSource(
            "No ID or source provided for import".into(),
        ))
    }

    /// Import using an ID importer
    async fn import_by_id(self, id: String) -> ImportResult {
        // If an importer name was provided, use that specific importer
        if let Some(importer_name) = self.importer_name {
            let importer = get_importer_by_name(&importer_name)
                .ok_or_else(|| ImportError::ImporterNotFound(importer_name.clone()))?;

            // This is a bit hacky - we're trying to use type_name to determine if it's an IdImporter
            let type_name = importer.name();
            if !type_name.contains("IdImporter") && !type_name.contains("NotUltranxImporter") {
                return Err(ImportError::ImporterNotFound(format!(
                    "Importer {} is not an IdImporter",
                    importer_name.clone()
                )));
            }

            // We know it's an importer that can handle IDs, proceed
            return start_import_by_id(importer, id).await;
        }

        // No importer specified, so we need to find one that can handle this ID
        let importer = find_id_importer_for(&id)
            .map_err(ImportError::from)?
            .ok_or_else(|| ImportError::NoSuitableImporterForId(id.clone()))?;

        start_import_by_id(importer, id).await
    }

    /// Import from a source path or URL
    async fn import_from_source(self, source: String) -> ImportResult {
        // Currently, we only support importing by ID
        // This is where you'd add FileImporter support when needed

        Err(ImportError::InvalidSource(
            "File importers not implemented yet, please use ID importers".into(),
        ))
    }
}

/// Start an import operation using an ID importer
async fn start_import_by_id(importer: Arc<dyn ImporterProvider>, id: String) -> ImportResult {
    // Get the actual importer ID from the registry
    let importer_id = match get_importer_id_from_provider(&importer) {
        Some(id) => id,
        None => {
            return Err(ImportError::ImporterNotFound(format!(
                "Could not get importer ID for: {}",
                importer.name()
            )));
        }
    };

    // Look up the importer by ID in the registry (this ensures we get a fresh instance)
    let importer = match crate::import::registry::get_importer_by_name(&importer_id) {
        Some(provider) => provider,
        None => {
            return Err(ImportError::ImporterNotFound(format!(
                "Importer not found in registry: {}",
                importer_id
            )));
        }
    };

    // Create a dynamic dispatch call to the appropriate IdImporter implementation
    // This uses a separate helper to keep this function clean
    process_id_import(importer, id).await
}

/// Get the registered importer ID for a provider
fn get_importer_id_from_provider(provider: &Arc<dyn ImporterProvider>) -> Option<String> {
    // Try to find the registered ID for this importer type
    let binding = crate::import::registry::registry();
    let registry = binding.read().unwrap();

    for (id, registered_provider) in &registry.providers_by_name {
        if registered_provider.name() == provider.name() {
            return Some(id.clone());
        }
    }

    None
}

/// Process an import using dynamic dispatch to the appropriate IdImporter implementation
async fn process_id_import(provider: Arc<dyn ImporterProvider>, id: String) -> ImportResult {
    // Use our new IdImportProviderObj wrapper to handle the dynamic dispatch
    if let Some(id_provider_obj) = IdImportProviderObj::try_from_provider(provider.clone()) {
        // Use the type-erased import method
        match id_provider_obj.import_by_id_string(&id).await {
            Ok(import_source) => {
                // Spawn a background task to handle the import
                let id_clone = id.clone();
                tokio::spawn(async move {
                    info!("Starting import for ID: {}", id_clone);
                    if let Err(e) = import_source.import().await {
                        error!("Failed to import game: {}", e);
                    } else {
                        info!("Import completed successfully for ID: {}", id_clone);
                    }
                    // Trigger a rescan after import
                    info!("Triggering rescan after import");
                    let _ = trigger_rescan(Default::default()).await;
                });

                Ok(Json(serde_json::json!({
                    "status": "success",
                    "message": "Import started",
                    "id": id
                }))
                .into_response())
            }
            Err(e) => Err(ImportError::from(e)),
        }
    } else {
        // Provider doesn't support ID-based importing
        Err(ImportError::InvalidSource(format!(
            "Importer {} does not support ID-based importing",
            provider.name()
        )))
    }
}

/// Helper function to easily import by ID with a specific importer
pub async fn import_by_id(importer_name: Option<String>, id: String) -> ImportResult {
    let request = match importer_name {
        Some(name) => ImportRequest::new_id_with_importer(name, id),
        None => ImportRequest::new_id(id),
    };

    request.import().await
}

/// Helper function to easily import from a source with a specific importer
pub async fn import_from_source(importer_name: Option<String>, source: String) -> ImportResult {
    let request = match importer_name {
        Some(name) => ImportRequest::new_source_with_importer(name, source),
        None => ImportRequest::new_source(source),
    };

    request.import().await
}
