use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub device_name: String,
    pub sample_rate: u32,
    /// Number of channels (2 for stereo, up to 64 for multi-channel Dante flows)
    pub channels: u32,
    pub latency_ms: u32,
    pub log_level: String,
    pub wasapi_exclusive: bool,
    /// Friendly names for TX channels (empty = use default "Ch 1", "Ch 2", etc.)
    pub channel_names: Vec<String>,
    /// Device lock to prevent unauthorized configuration changes
    pub device_locked: bool,
    /// Network interface to bind to (empty string = auto-detect)
    pub network_interface: String,
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
            channel_names: vec![],
            device_locked: false,
            network_interface: String::new(),
        }
    }
}

impl Config {
    /// Get friendly name for channel (uses config names or defaults like "Ch 1", "Ch 2")
    pub fn channel_name(&self, idx: usize) -> String {
        self.channel_names.get(idx)
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("Ch {}", idx + 1))
    }

    pub fn resolve_interface_ip(&self) -> Option<std::net::Ipv4Addr> {
        if self.network_interface.is_empty() {
            return None; // use system default
        }
        // Try to parse as IP address first
        if let Ok(ip) = self.network_interface.parse::<std::net::Ipv4Addr>() {
            return Some(ip);
        }
        // Otherwise treat as interface name — log it
        tracing::info!("Network interface '{}' specified — IP resolution not yet implemented", self.network_interface);
        None
    }

    pub fn config_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("inferno_aoip")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(s) => match toml::from_str(&s) {
                    Ok(c) => {
                        tracing::info!("Loaded config from {}", path.display());
                        c
                    }
                    Err(e) => {
                        tracing::warn!("Config parse error: {e}, using defaults");
                        Config::default()
                    }
                },
                Err(e) => {
                    tracing::warn!("Could not read config: {e}, using defaults");
                    Config::default()
                }
            }
        } else {
            Config::default()
        };

        // Validate channel count
        if config.channels == 0 || config.channels > 64 {
            tracing::warn!("Invalid channel count {} in config, clamping to 2", config.channels);
            config.channels = 2;
        }

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
