//! Generic KV configuration based on SurrealDB.
//!
//! This module provides a SurrealDB table called `settings` to store key-value pairs.
//!
//!
//! The values are `key` and `value` pairs, where `key` is a string and `value` is a JSON object.

// The returning value should return a serde json value

use crate::{db::DB, index::SourceList};
use color_eyre::Result;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
const TABLE_NAME: &str = "settings";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]

pub struct KVConfig {
    pub key: String,
    pub value: Option<Value>,
}

impl KVConfig {
    pub fn new(key: String, value: Option<Value>) -> Self {
        Self { key, value }
    }

    pub async fn get(key: &str) -> Result<Option<Self>> {
        let q = DB.select((TABLE_NAME, key)).await?;
        tracing::trace!("Retrieved value for key {}: {:?}", key, q);

        if q.is_none() {
            tracing::warn!("Key {} not found in KVConfig", key);
            return Ok(None);
        }
        Ok(q)
    }

    pub async fn set(&mut self, value: Value) -> Result<()> {
        self.value = Some(value.clone()); // Keep the struct's state consistent locally
        let key = &self.key;
        let q: Option<Self> = DB.upsert((TABLE_NAME, key)).content(self.clone()).await?;
        tracing::trace!("Set value for key {}: {:?}", key, q);
        Ok(())
    }

    /// Delete the key-value pair from the database, consuming the struct
    pub async fn delete(self) -> Result<()> {
        let key = &self.key;
        let q: Option<Self> = DB.delete((TABLE_NAME, key)).await?;
        tracing::trace!("Deleted value for key {}: {:?}", key, q);
        Ok(())
    }
}

// Now let's implement a trait that turns things into a KVConfig from its JSON representation
/// A trait for types that can be stored in a KV store.
/// This trait provides methods to serialize, deserialize, and manage configuration items.
pub trait KvOptExt: Serialize + DeserializeOwned + Clone + Default {
    /// Returns the key name for this configuration item.
    const KEY_NAME: &'static str;

    /// Provides a default value if the key is not found.
    /// This is now an associated function, not requiring `self`.
    fn default_value() -> Option<Self>
    where
        Self: Sized,
    {
        // Default implementation returns the struct's Default value
        Some(Self::default())
    }

    /// Serializes the implementing struct and saves it to the KV store
    /// using the key provided by `KEY_NAME`.
    async fn set(&self) -> Result<()> {
        let key = Self::KEY_NAME;
        let value = serde_json::to_value(self)?;
        // Create a KVConfig instance to manage the database operation.
        let mut kv_config = KVConfig::new(key.to_string(), None);
        kv_config.set(value).await?;
        Ok(())
    }

    /// Retrieves the value from the KV store using the key provided by `KEY_NAME`.
    ///
    /// If the key is not found and a default value is available (`default_value()` returns `Some`),
    /// the default value is automatically saved to the store before being returned.
    ///
    /// Returns `Ok(Some(Self))` if found or a default is available and set.
    ///
    /// Returns `Ok(None)` if not found and no default is available.
    ///
    /// Returns `Err` if there's a database, serialization, or deserialization error.
    async fn get() -> Result<Option<Self>>
    where
        Self: Sized + std::fmt::Debug, // Add Debug constraint for tracing
    {
        let key = Self::KEY_NAME;
        let value_opt_result = KVConfig::get(key).await;

        match value_opt_result {
            Ok(Some(kv_config)) => {
                // Entry found, check if value exists
                if let Some(value) = kv_config.value {
                    // Attempt to deserialize the value
                    match serde_json::from_value::<Self>(value.clone()) {
                        // Clone value for potential error logging
                        Ok(deserialized_value) => {
                            tracing::trace!(
                                "Successfully deserialized value for key '{}': {:?}",
                                key,
                                deserialized_value
                            );
                            return Ok(Some(deserialized_value));
                        }
                        Err(e) => {
                            // Deserialization failed, log the error and proceed to default handling
                            tracing::error!(
                                "Failed to deserialize value {:?} for key '{}': {}. Falling back to default.",
                                value, // Log the problematic value
                                key,
                                e
                            );
                            // Fall through to default handling below
                        }
                    }
                } else {
                    // Entry exists but value is None (unexpected state), log and proceed to default
                    tracing::warn!(
                        "KVConfig found for key '{}' but its value is None. Falling back to default.",
                        key
                    );
                    // Fall through to default handling below
                }
            }
            Ok(None) => {
                // Key not found in the database, proceed to default handling
                tracing::debug!("Key '{}' not found in KVConfig. Checking for default.", key);
                // Fall through to default handling below
            }
            Err(e) => {
                // Database error occurred during get
                tracing::error!("Failed to get key '{}' from KVConfig: {}", key, e);
                return Err(e); // Propagate the error
            }
        }

        // Handle default value logic (reached if key not found, value was None, or deserialization failed)
        if let Some(default_value) = Self::default_value() {
            tracing::info!("Using default value for key '{}' and saving it.", key);
            // Serialize the default value
            let value_to_set = serde_json::to_value(&default_value)?;
            // Create a KVConfig instance to save the default
            let mut kv_config_to_set = KVConfig::new(key.to_string(), None);
            // Save the default value to the database, propagating potential errors
            kv_config_to_set.set(value_to_set).await?;
            // Log the default value being returned
            tracing::trace!(
                "Returning default value for key '{}': {:?}",
                key,
                default_value
            );
            // Return the default value
            Ok(Some(default_value))
        } else {
            // No value found and no default value provided
            tracing::warn!("Key '{}' not found and no default value is available.", key);
            Ok(None)
        }
    }

    /// Deletes the key-value pair from the KV store using the key provided by `KEY_NAME`.
    ///
    /// Next time you call `get()`, it will now be re-created with the default value.
    async fn delete(self) -> Result<()> {
        let key = Self::KEY_NAME;
        let kv_config = KVConfig::new(key.to_string(), None);
        kv_config.delete().await?;
        Ok(())
    }
}

// Let's do an example setting option?
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Motd {
    #[serde(default)] // Add this attribute
    pub message: Option<String>,
    pub enabled: bool,
}

impl KvOptExt for Motd {
    const KEY_NAME: &'static str = "motd";
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtraSourcesConfig {
    pub sources: Vec<SourceList>,
}

impl KvOptExt for ExtraSourcesConfig {
    const KEY_NAME: &'static str = "extra_sources";
}
