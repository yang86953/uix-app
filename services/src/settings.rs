use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uix_platform::{Error, Errc, Result};

/// Key-value settings service with JSON persistence.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct SettingsService {
    path: Option<String>,
    values: HashMap<String, String>,
    dirty: bool,
}


impl SettingsService {
    pub fn new() -> Self { Self::default() }

    /// Load settings from a JSON file.
    pub fn load(&mut self, path: &str) -> Result<()> {
        self.path = Some(path.to_string());
        self.values.clear();

        if !Path::new(path).exists() {
            return Ok(());
        }

        let content = fs::read_to_string(path)?;
        if content.trim().is_empty() {
            return Ok(());
        }

        let parsed: HashMap<String, serde_json::Value> =
            serde_json::from_str(&content)
                .map_err(|e| Error::new(Errc::FormatError, format!("failed to parse settings: {}", e)))?;

        for (k, v) in parsed {
            let str_val = match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            self.values.insert(k, str_val);
        }

        self.dirty = false;
        Ok(())
    }

    /// Save settings to the JSON file.
    pub fn save(&mut self) -> Result<()> {
        let path = self.path.as_deref()
            .ok_or_else(|| Error::new(Errc::InvalidState, "no settings path set"))?;

        let json = serde_json::to_string_pretty(&self.values)
            .map_err(|e| Error::new(Errc::FormatError, format!("failed to serialize settings: {}", e)))?;

        fs::write(path, &json)?;
        self.dirty = false;
        Ok(())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
        self.dirty = true;
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.values.get(key).map(|s| s.as_str()).unwrap_or(default)
    }

    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.dirty = true;
    }

    pub fn all(&self) -> &HashMap<String, String> { &self.values }
    pub fn dirty(&self) -> bool { self.dirty }
    pub fn count(&self) -> usize { self.values.len() }
}
