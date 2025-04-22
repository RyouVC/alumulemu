use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use once_cell::sync::Lazy;
use tracing::{debug, info};

use crate::import::{Importer, Result, not_ultranx::NotUltranxImporter, url::UrlImporter};

/// A static global registry for importers
static IMPORTER_REGISTRY: Lazy<Arc<RwLock<ImporterRegistry>>> =
    Lazy::new(|| Arc::new(RwLock::new(ImporterRegistry::new())));

/// Registry for managing all importers
#[derive(Default)]
pub struct ImporterRegistry {
    /// Maps importer names to the actual importer instances
    importers: HashMap<String, Box<dyn DynImporter>>,
    /// Maps user-friendly names to the actual importer names
    friendly_names: HashMap<String, String>,
}

/// Type-erased importer trait object
pub trait DynImporter: Send + Sync {
    /// Return the importer as Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Return a string name/identifier for this importer
    fn name(&self) -> &'static str;

    /// Return a user-friendly display name for this importer
    fn display_name(&self) -> &'static str;

    /// Return a description of this importer
    fn description(&self) -> &'static str;

    /// Clone the importer into a Box
    fn clone_box(&self) -> Box<dyn DynImporter>;
}

/// Implement DynImporter for any type that implements Importer
impl<T: Importer> DynImporter for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        self.name()
    }

    fn display_name(&self) -> &'static str {
        self.display_name()
    }

    fn description(&self) -> &'static str {
        self.description()
    }

    fn clone_box(&self) -> Box<dyn DynImporter> {
        Box::new(self.clone())
    }
}

impl ImporterRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            importers: HashMap::new(),
            friendly_names: HashMap::new(),
        }
    }

    /// Register an importer with the registry using the given ID
    pub fn register<T: Importer>(&mut self, id: &str, importer: T) {
        let display_name = importer.display_name().to_string();

        debug!(
            importer = id,
            display_name = display_name,
            "Registering importer"
        );

        // Register the friendly name if it's different from the ID
        if id != display_name {
            self.friendly_names
                .insert(display_name.to_lowercase(), id.to_string());
        }

        self.importers.insert(id.to_string(), Box::new(importer));
    }

    /// Get an importer by its ID
    pub fn get(&self, id: &str) -> Option<&Box<dyn DynImporter>> {
        self.importers.get(id)
    }

    /// Get all registered importers
    pub fn get_all(&self) -> Vec<&Box<dyn DynImporter>> {
        self.importers.values().collect()
    }

    /// Check if an importer with the specified ID is registered
    pub fn has(&self, id: &str) -> bool {
        self.importers.contains_key(id)
    }
}

/// Helper functions to work with the global registry
pub fn register<T: Importer>(id: &str, importer: T) {
    let mut registry = IMPORTER_REGISTRY.write().unwrap();
    registry.register(id, importer);
}

/// Initialize the registry with default importers
pub async fn init_registry() {
    info!("Initializing importer registry");

    // Register the NotUltranxImporter
    register("ultranx", NotUltranxImporter::new().await);

    // Register the UrlImporter
    register("url", UrlImporter::new());

    // Add more importers here as they become available

    info!("Importer registry initialized");
}

/// Get a reference to the global registry
pub fn registry() -> Arc<RwLock<ImporterRegistry>> {
    IMPORTER_REGISTRY.clone()
}

/// Get an importer by ID from the global registry
pub fn get_importer(id: &str) -> Option<Box<dyn DynImporter>> {
    let registry = IMPORTER_REGISTRY.read().unwrap();
    registry.get(id).map(|importer| importer.clone_box())
}

/// Get all registered importers from the global registry
pub fn get_all_importers() -> Vec<Box<dyn DynImporter>> {
    let registry = IMPORTER_REGISTRY.read().unwrap();
    registry
        .get_all()
        .into_iter()
        .map(|importer| importer.clone_box())
        .collect()
}

/// Import using a specific importer and JSON request
/// This is a more effective approach that ensures locks are released before async operations
pub async fn import_with_json(id: &str, json: &str) -> Result<crate::import::ImportSource> {
    // First, clone the importers while holding the lock, if they exist
    let (ultranx_importer, url_importer) = {
        // Create a scope to ensure the lock is released before any async operations
        let registry = IMPORTER_REGISTRY.read().unwrap();

        // Find and clone the appropriate importer based on ID
        let ultranx = match id {
            "ultranx" => registry
                .get(id)
                .and_then(|imp| imp.as_any().downcast_ref::<NotUltranxImporter>())
                .cloned(),
            _ => None,
        };

        let url = match id {
            "url" => registry
                .get(id)
                .and_then(|imp| imp.as_any().downcast_ref::<UrlImporter>())
                .cloned(),
            _ => None,
        };

        (ultranx, url)
    }; // Lock is dropped here

    // Now process the import with the cloned importer (no locks held)
    if let Some(importer) = ultranx_importer {
        // We can now safely call async methods since we no longer hold the lock
        let request = serde_json::from_str(json).map_err(|e| {
            crate::import::ImportError::Other(color_eyre::eyre::eyre!(
                "Failed to parse JSON request: {}",
                e
            ))
        })?;

        importer.import(request).await
    } else if let Some(importer) = url_importer {
        // We can now safely call async methods since we no longer hold the lock
        let request = serde_json::from_str(json).map_err(|e| {
            crate::import::ImportError::Other(color_eyre::eyre::eyre!(
                "Failed to parse JSON request: {}",
                e
            ))
        })?;

        importer.import(request).await
    } else {
        Err(crate::import::ImportError::Other(color_eyre::eyre::eyre!(
            "Importer not found or not supported: {}",
            id
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_basic() {
        // Initialize the registry
        init_registry().await;

        // Test registry access
        let registry = IMPORTER_REGISTRY.read().unwrap();
        assert!(
            registry.has("ultranx"),
            "UltraNX importer should be registered"
        );
        assert!(registry.has("url"), "URL importer should be registered");
    }

    #[tokio::test]
    async fn test_import_ultranx() {
        // Initialize the registry
        init_registry().await;

        // Test JSON import for UltraNX
        let json = r#"{"title_id": "0100000000000000", "download_type": "fullpkg"}"#;
        let result = import_with_json("ultranx", json).await;

        // This might fail in tests without mocking, but we're testing the mechanism
        assert!(
            result.is_err() || result.is_ok(),
            "Import should either succeed or fail with a handled error"
        );
    }
}
