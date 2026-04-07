use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub latency_ms: u32,
    pub log_level: String,
    pub wasapi_exclusive: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_name: "InfernoAoIP".to_string(),
            sample_rate: 48000,
            channels: 2,
            latency_ms: 10,
            log_level: "info".to_string(),
            wasapi_exclusive: false,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("inferno_aoip")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(s) => match toml::from_str(&s) {
                    Ok(c) => {
                        tracing::info!("Loaded config from {}", path.display());
                        return c;
                    }
                    Err(e) => tracing::warn!("Config parse error: {e}, using defaults"),
                },
                Err(e) => tracing::warn!("Could not read config: {e}, using defaults"),
            }
        }
        let config = Config::default();
        config.save(); // write defaults on first run
        config
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        match toml::to_string_pretty(self) {
            Ok(s) => {
                std::fs::write(&path, s).ok();
            }
            Err(e) => tracing::error!("Could not serialize config: {e}"),
        }
    }
}
