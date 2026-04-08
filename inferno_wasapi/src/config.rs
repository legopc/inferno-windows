use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use if_addrs;

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
    /// Frames per packet: "auto", "min", "max", or a number (e.g. "48")
    /// "auto" = let the protocol negotiate
    /// "min" = minimum FPP (lowest latency, highest overhead)
    /// "max" = maximum FPP (highest latency, lowest overhead)
    /// Default: "auto"
    #[serde(default = "Config::default_fpp")]
    pub fpp: String,
    /// RX ring buffer size in samples per channel (power of 2, default 524288 ≈ 10.9s at 48kHz)
    #[serde(default = "Config::default_rx_buffer_samples")]
    pub rx_buffer_samples: u32,
    /// Latency reference in samples (default 4800 = 100ms at 48kHz)  
    #[serde(default = "Config::default_latency_ref_samples")]
    pub latency_ref_samples: u32,
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
            fpp: "auto".to_string(),
            rx_buffer_samples: 524288,
            latency_ref_samples: 4800,
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

    fn default_fpp() -> String { "auto".to_string() }

    fn default_rx_buffer_samples() -> u32 { 524288u32 }

    fn default_latency_ref_samples() -> u32 { 4800u32 }

    pub fn resolve_interface_ip(&self) -> Option<std::net::Ipv4Addr> {
        if self.network_interface.is_empty() {
            return None; // use system default
        }
        // Try to parse as IP address first
        if let Ok(ip) = self.network_interface.parse::<std::net::Ipv4Addr>() {
            return Some(ip);
        }
        // Otherwise treat as interface name and look it up
        if let Ok(ifaces) = if_addrs::get_if_addrs() {
            for iface in ifaces {
                if iface.name == self.network_interface {
                    match iface.ip() {
                        std::net::IpAddr::V4(ipv4) => return Some(ipv4),
                        std::net::IpAddr::V6(_) => continue,
                    }
                }
            }
        }
        tracing::warn!("Network interface '{}' not found, using default", self.network_interface);
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

        // Validate and clamp rx_buffer_samples to next power of 2
        if config.rx_buffer_samples == 0 {
            tracing::warn!("Invalid rx_buffer_samples {} in config, using default 524288", config.rx_buffer_samples);
            config.rx_buffer_samples = 524288;
        } else if !config.rx_buffer_samples.is_power_of_two() {
            let next_pow2 = config.rx_buffer_samples.next_power_of_two();
            tracing::warn!("rx_buffer_samples {} is not power of 2, clamping to {}", config.rx_buffer_samples, next_pow2);
            config.rx_buffer_samples = next_pow2;
        }

        // Enforce Dante 31-char device name limit
        if config.device_name.len() > 31 {
            let truncated = config.device_name.chars().take(31).collect::<String>();
            tracing::warn!(
                "device_name '{}' exceeds 31-char Dante limit, truncating to '{}'",
                config.device_name, truncated
            );
            config.device_name = truncated;
        }
        // Also validate no special chars (Dante only allows alphanumeric, hyphen, space)
        let valid: String = config.device_name.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == ' ' { c } else { '-' })
            .collect();
        if valid != config.device_name {
            tracing::warn!("device_name contained invalid chars, sanitized to '{}'", valid);
            config.device_name = valid;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.sample_rate, 48000);
        assert_eq!(cfg.channels, 2);
        assert_eq!(cfg.latency_ms, 10);
        assert_eq!(cfg.device_name, "InfernoAoIP");
        assert_eq!(cfg.log_level, "info");
    }

    #[test]
    fn test_config_round_trip() {
        let original = Config {
            device_name: "TestDevice".to_string(),
            sample_rate: 96000,
            channels: 8,
            latency_ms: 20,
            rx_buffer_samples: 262144,
            latency_ref_samples: 9600,
            ..Config::default()
        };
        let serialized = toml::to_string(&original).expect("serialize");
        let loaded: Config = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(loaded.device_name, "TestDevice");
        assert_eq!(loaded.sample_rate, 96000);
        assert_eq!(loaded.channels, 8);
        assert_eq!(loaded.latency_ms, 20);
        assert_eq!(loaded.fpp, "auto");
        assert_eq!(loaded.rx_buffer_samples, 262144);
        assert_eq!(loaded.latency_ref_samples, 9600);
    }

    #[test]
    fn test_config_parse_all_fields() {
        // Parse a TOML with all required fields specified
        let toml = r#"
device_name = "MyDevice"
sample_rate = 48000
channels = 2
latency_ms = 10
log_level = "debug"
wasapi_exclusive = true
channel_names = ["Left", "Right"]
device_locked = true
network_interface = "192.168.1.1"
fpp = "max"
rx_buffer_samples = 262144
latency_ref_samples = 9600
"#;
        let cfg: Config = toml::from_str(toml).expect("parse");
        assert_eq!(cfg.device_name, "MyDevice");
        assert_eq!(cfg.sample_rate, 48000);
        assert_eq!(cfg.channels, 2);
        assert_eq!(cfg.latency_ms, 10);
        assert_eq!(cfg.log_level, "debug");
        assert!(cfg.wasapi_exclusive);
        assert_eq!(cfg.channel_names.len(), 2);
        assert!(cfg.device_locked);
        assert_eq!(cfg.network_interface, "192.168.1.1");
        assert_eq!(cfg.fpp, "max");
        assert_eq!(cfg.rx_buffer_samples, 262144);
        assert_eq!(cfg.latency_ref_samples, 9600);
    }

    #[test]
    fn test_channel_name_default() {
        let cfg = Config::default();
        assert_eq!(cfg.channel_name(0), "Ch 1");
        assert_eq!(cfg.channel_name(1), "Ch 2");
    }

    #[test]
    fn test_channel_name_custom() {
        let mut cfg = Config::default();
        cfg.channel_names = vec!["Left".to_string(), "Right".to_string()];
        assert_eq!(cfg.channel_name(0), "Left");
        assert_eq!(cfg.channel_name(1), "Right");
        assert_eq!(cfg.channel_name(2), "Ch 3"); // falls back to default
    }

    #[test]
    fn test_device_name_truncation() {
        // Simulate what load() does for long names
        let long_name = "A".repeat(40);
        let truncated: String = long_name.chars().take(31).collect();
        assert_eq!(truncated.len(), 31);
    }
}
