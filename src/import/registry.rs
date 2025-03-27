use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, RwLock},
};

use once_cell::sync::Lazy;
use tracing::{debug, info};

use crate::import::{Importer, Result, not_ultranx::NotUltranxImporter, url::UrlImporter};

/// A static global registry for importers
static IMPORTER_REGISTRY: Lazy<Arc<RwLock<ImporterRegistry>>> =
    Lazy::new(|| Arc::new(RwLock::new(ImporterRegistry::new())));

/// Trait for abstracting over different importer types in our registry
pub trait ImporterProvider: Send + Sync + 'static {
    /// Return the importer as Any for downcasting
    fn as_any(&self) -> &dyn Any;
    /// Return a string name/identifier for this importer
    fn name(&self) -> &'static str;
    /// Return a user-friendly display name for this importer
    fn display_name(&self) -> &'static str;
    /// Return a description of this importer
    fn description(&self) -> &'static str;
    /// Clone the provider into a Box
    fn clone_box(&self) -> Box<dyn ImporterProvider>;
}

/// Trait for types that can provide custom names to override the default
pub trait CustomImporterName {
    /// Get the custom name for this importer
    fn custom_name(&self) -> &'static str;

    /// Get the custom display name for this importer
    fn custom_display_name(&self) -> &'static str;

    /// Get the custom description for this importer
    fn custom_description(&self) -> &'static str;
}

// Implementation for any type that implements Importer + Clone
impl<T: Importer + Clone + Send + Sync + 'static> ImporterProvider for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn display_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn description(&self) -> &'static str {
        "An importer implementation"
    }

    fn clone_box(&self) -> Box<dyn ImporterProvider> {
        Box::new(self.clone())
    }
}

/// Registry for managing all importers
#[derive(Default)]
pub struct ImporterRegistry {
    providers: HashMap<TypeId, Arc<dyn ImporterProvider>>,
    pub providers_by_name: HashMap<String, Arc<dyn ImporterProvider>>,
    /// Maps user-friendly names to the actual provider names
    friendly_names: HashMap<String, String>,
}

impl ImporterRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            providers_by_name: HashMap::new(),
            friendly_names: HashMap::new(),
        }
    }

    /// Register an importer with the registry using the default name
    pub fn register<T: ImporterProvider>(&mut self, provider: T) {
        let provider = Arc::new(provider);
        let type_id = provider.as_any().type_id();
        let name = provider.name().to_string();
        let display_name = provider.display_name().to_string();

        debug!(
            importer = name,
            display_name = display_name,
            "Registering importer"
        );

        // Register the friendly name if it's different from the type name
        if name != display_name {
            self.friendly_names
                .insert(display_name.to_lowercase(), name.clone());
        }

        self.providers.insert(type_id, provider.clone());
        self.providers_by_name.insert(name, provider);
    }

    /// Register an importer with the registry using a custom ID
    pub fn register_with_id<T: ImporterProvider>(&mut self, id: &str, provider: T) {
        let provider = Arc::new(provider);
        let type_id = provider.as_any().type_id();
        let display_name = provider.display_name().to_string();

        debug!(
            importer = id,
            display_name = display_name,
            "Registering importer with custom ID"
        );

        // Register the friendly name mapping to the custom ID
        if id != display_name {
            self.friendly_names
                .insert(display_name.to_lowercase(), id.to_string());
        }

        self.providers.insert(type_id, provider.clone());
        self.providers_by_name.insert(id.to_string(), provider);
    }

    /// Get an importer by its type
    pub fn get<T: 'static + Clone>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        self.providers.get(&type_id).and_then(|provider| {
            // Properly downcast to the concrete type
            provider.as_any().downcast_ref::<T>().map(|t| {
                // Create a new instance by cloning
                Arc::new((*t).clone())
            })
        })
    }

    /// Get an importer by its name
    pub fn get_by_name(&self, name: &str) -> Option<Arc<dyn ImporterProvider>> {
        self.providers_by_name.get(name).cloned()
    }

    /// Get all registered importers
    pub fn get_all(&self) -> Vec<Arc<dyn ImporterProvider>> {
        self.providers_by_name.values().cloned().collect()
    }

    /// Check if a specific importer type is registered
    pub fn has<T: 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        self.providers.contains_key(&type_id)
    }

    /// Check if an importer with the specified name is registered
    pub fn has_by_name(&self, name: &str) -> bool {
        self.providers_by_name.contains_key(name)
    }
}

/// Helper functions to work with the global registry
pub fn register<T: ImporterProvider>(provider: T) {
    let mut registry = IMPORTER_REGISTRY.write().unwrap();
    registry.register(provider);
}

/// Register an importer with a custom ID
pub fn register_with_id<T: ImporterProvider>(id: &str, provider: T) {
    let mut registry = IMPORTER_REGISTRY.write().unwrap();
    registry.register_with_id(id, provider);
}

/// Initialize the registry with default importers
pub fn init_registry() {
    info!("Initializing importer registry");

    // Register the NotUltranxImporter with a custom ID
    register_with_id("ultranx", NotUltranxImporter::new());

    // Register the UrlImporter with a custom ID
    register_with_id("url", UrlImporter::new());

    // Add more importers here as they become available

    info!("Importer registry initialized");
}

/// Get a reference to the global registry
pub fn registry() -> Arc<RwLock<ImporterRegistry>> {
    IMPORTER_REGISTRY.clone()
}

/// Get an importer by type from the global registry
pub fn get_importer<T: 'static + Clone>() -> Option<Arc<T>> {
    let registry = IMPORTER_REGISTRY.read().unwrap();
    registry.get::<T>()
}

/// Get an importer by name from the global registry
pub fn get_importer_by_name(name: &str) -> Option<Arc<dyn ImporterProvider>> {
    let registry = IMPORTER_REGISTRY.read().unwrap();
    registry.get_by_name(name)
}

/// Get all registered importers from the global registry
pub fn get_all_importers() -> Vec<Arc<dyn ImporterProvider>> {
    let registry = IMPORTER_REGISTRY.read().unwrap();
    registry.get_all()
}

/// Helper trait for working with FileImporters
pub trait ImporterRegistryFileExt {
    /// Find all registered FileImporters
    fn get_file_importers(&self) -> Vec<Arc<dyn ImporterProvider>>;
}

/// Helper trait for working with IdImporters
pub trait ImporterRegistryIdExt {
    /// Find all registered IdImporters
    fn get_id_importers(&self) -> Vec<Arc<dyn ImporterProvider>>;

    /// Try to find an id importer that can handle the given id
    fn find_id_importer_for(&self, id: &str) -> Result<Option<Arc<dyn ImporterProvider>>>;
}

impl ImporterRegistryFileExt for ImporterRegistry {
    fn get_file_importers(&self) -> Vec<Arc<dyn ImporterProvider>> {
        // This is a simplistic approach - in practice, you might want to tag importers
        // or use a more sophisticated approach to identify FileImporters
        self.get_all()
            .into_iter()
            .filter(|provider| {
                // Attempt to downcast to determine if this is a FileImporter
                // This is a bit of a hack but works for demonstration
                let type_name = provider.name();
                // Check if the type name contains any indication it's a FileImporter
                type_name.contains("FileImporter") || type_name.contains("file_importer")
            })
            .collect()
    }
}

impl ImporterRegistryIdExt for ImporterRegistry {
    fn get_id_importers(&self) -> Vec<Arc<dyn ImporterProvider>> {
        // Similar approach to get_file_importers
        self.get_all()
            .into_iter()
            .filter(|provider| {
                let type_name = provider.name();
                type_name.contains("IdImporter") || 
                // Known implementations
                type_name.contains("NotUltranxImporter") ||
                type_name.contains("UrlImporter")
            })
            .collect()
    }

    fn find_id_importer_for(&self, id: &str) -> Result<Option<Arc<dyn ImporterProvider>>> {
        // Try to determine the appropriate importer based on the ID format

        // First, check if this might be a URL-encoded URL
        if id.contains("%3A%2F%2F") || (id.starts_with("http") && id.contains("%")) {
            // Likely a URL-encoded URL
            debug!(id = id, "Detected URL-encoded input, using UrlImporter");
            return Ok(self.get_by_name("url"));
        }

        // Check for title IDs in traditional format (ultranx case)
        if id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit()) {
            debug!(
                id = id,
                "Detected title ID format, using NotUltranxImporter"
            );
            return Ok(self.get_by_name("ultranx"));
        }

        // For other formats, try to find a suitable importer
        debug!(id = id, "No specific importer detected for ID format");

        // Return the first suitable importer as a fallback
        Ok(self.get_id_importers().into_iter().next())
    }
}

// Extension functions for the global registry
pub fn get_file_importers() -> Vec<Arc<dyn ImporterProvider>> {
    let registry = IMPORTER_REGISTRY.read().unwrap();
    registry.get_file_importers()
}

pub fn get_id_importers() -> Vec<Arc<dyn ImporterProvider>> {
    let registry = IMPORTER_REGISTRY.read().unwrap();
    registry.get_id_importers()
}

pub fn find_id_importer_for(id: &str) -> Result<Option<Arc<dyn ImporterProvider>>> {
    let registry = IMPORTER_REGISTRY.read().unwrap();
    registry.find_id_importer_for(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ImportSource, url::UrlImporter};

    #[tokio::test]
    async fn test_url_importer_registration() {
        // Initialize the registry
        init_registry();

        // Check that our URL importer is registered
        let url_importer = get_importer_by_name("url");
        assert!(url_importer.is_some(), "URL importer should be registered");

        // Test auto-detection for a URL-encoded URL
        let encoded_url = "https%3A%2F%2Fexample.com%2Fgame.nsp";
        let importer = find_id_importer_for(encoded_url).unwrap();
        assert!(
            importer.is_some(),
            "Should find an importer for URL-encoded input"
        );

        // Make sure it's the URL importer that was selected
        let importer_name = importer.unwrap().name();
        assert!(
            importer_name.contains("UrlImporter"),
            "Selected importer should be UrlImporter, got: {}",
            importer_name
        );
    }

    // Existing tests...
    use crate::import::{FileImporter, not_ultranx::NotUltranxImporter};

    // A simple mock importer for testing
    #[derive(Clone)]
    struct MockFileImporter;

    #[derive(Debug, Default)]
    struct MockImportOptions;

    impl Importer for MockFileImporter {
        type ImportOptions = MockImportOptions;
    }

    impl FileImporter for MockFileImporter {
        async fn import_from_source(
            &self,
            _source: &std::path::Path,
            _options: Option<Self::ImportOptions>,
        ) -> crate::import::Result<crate::import::ImportSource> {
            // Mock implementation - just return a placeholder
            Ok(crate::import::ImportSource::Local(
                std::path::PathBuf::from("/tmp/mock"),
            ))
        }
    }

    #[tokio::test]
    async fn test_registry_basic() {
        // Create a fresh registry for testing
        let mut registry = ImporterRegistry::new();

        // Register our importers
        registry.register(MockFileImporter);
        registry.register(NotUltranxImporter::new());

        // Test registry functionality
        assert_eq!(
            registry.get_all().len(),
            2,
            "Should have 2 registered importers"
        );

        // Test getting file importers
        let file_importers = registry.get_file_importers();
        assert_eq!(file_importers.len(), 1, "Should have 1 file importer");

        // Test getting id importers
        let id_importers = registry.get_id_importers();
        assert_eq!(id_importers.len(), 1, "Should have 1 id importer");
    }

    #[tokio::test]
    async fn test_global_registry() {
        // Initialize the global registry
        init_registry();

        // Test global registry access functions
        let all_importers = get_all_importers();
        assert!(
            !all_importers.is_empty(),
            "Global registry should have importers"
        );

        // Test ID importers access
        let id_importers = get_id_importers();
        assert!(
            !id_importers.is_empty(),
            "Should have ID importers registered"
        );

        // Test finder function
        let importer = find_id_importer_for("0100000000000000").unwrap();
        assert!(
            importer.is_some(),
            "Should find an importer for a valid ID format"
        );
    }
}
