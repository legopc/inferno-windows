use native_windows_gui as nwg;
use std::path::PathBuf;

/// Settings dialog window
/// Allows user to configure device name, sample rate, channels, latency, WASAPI device, NIC, and FPP settings
#[allow(dead_code)]
#[derive(Default)]
pub struct SettingsWindow {
    pub window: nwg::Window,

    // Labels
    pub lbl_device_name: nwg::Label,
    pub lbl_sample_rate: nwg::Label,
    pub lbl_channels: nwg::Label,
    pub lbl_latency: nwg::Label,
    pub lbl_wasapi_device: nwg::Label,
    pub lbl_network_interface: nwg::Label,
    pub lbl_fpp: nwg::Label,
    pub lbl_custom_fpp: nwg::Label,

    // Input controls
    pub txt_device_name: nwg::TextInput,
    pub combo_sample_rate: nwg::ComboBox<String>,
    pub txt_channels: nwg::TextInput,
    pub txt_latency: nwg::TextInput,
    pub combo_wasapi_device: nwg::ComboBox<String>,
    pub combo_network_interface: nwg::ComboBox<String>,
    pub combo_fpp: nwg::ComboBox<String>,
    pub txt_custom_fpp: nwg::TextInput,

    // Buttons
    pub btn_save: nwg::Button,
    pub btn_cancel: nwg::Button,
}

/// Enumerate network interfaces using if_addrs
pub fn list_network_interfaces() -> Vec<String> {
    match if_addrs::get_if_addrs() {
        Ok(ifaces) => ifaces
            .into_iter()
            .filter(|i| !i.is_loopback())
            .map(|i| format!("{} ({})", i.name, i.ip()))
            .collect(),
        Err(_) => vec!["Default".to_string()],
    }
}

/// Get current config file path
/// Mirrors Config::config_path() from inferno_wasapi
pub fn get_config_path() -> PathBuf {
    if let Some(local_data_dir) = dirs::data_local_dir() {
        local_data_dir.join("inferno_aoip").join("config.toml")
    } else {
        PathBuf::from("config.toml")
    }
}

/// Load current config values into the window
pub fn load_config_into_window(window: &SettingsWindow) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_config_path();
    if !path.exists() {
        return Err(format!("Config file not found at {:?}", path).into());
    }

    let content = std::fs::read_to_string(&path)?;
    let config: toml::Value = toml::from_str(&content)?;

    // Device Name
    if let Some(device_name) = config.get("device_name").and_then(|v| v.as_str()) {
        window.txt_device_name.set_text(device_name);
    }

    // Sample Rate
    if let Some(sample_rate) = config.get("sample_rate").and_then(|v| v.as_integer()) {
        let sr_str = sample_rate.to_string();
        if let Some(index) = window.combo_sample_rate.collection().iter().position(|s| s == &sr_str) {
            window.combo_sample_rate.set_selection(Some(index));
        }
    }

    // Channels
    if let Some(channels) = config.get("channels").and_then(|v| v.as_integer()) {
        window.txt_channels.set_text(&channels.to_string());
    }

    // Latency (ms)
    if let Some(latency) = config.get("latency_ms").and_then(|v| v.as_integer()) {
        window.txt_latency.set_text(&latency.to_string());
    }

    // Network Interface
    if let Some(nic) = config.get("network_interface").and_then(|v| v.as_str()) {
        // Try to find matching NIC by name
        for (idx, iface_str) in window.combo_network_interface.collection().iter().enumerate() {
            if iface_str.starts_with(nic) {
                window.combo_network_interface.set_selection(Some(idx));
                break;
            }
        }
    }

    // FPP Mode
    if let Some(fpp) = config.get("fpp").and_then(|v| v.as_str()) {
        let fpp_display = match fpp {
            "auto" => "Auto (negotiate)",
            "min" => "Min Latency (1 packet)",
            "max" => "Max Efficiency (64 packets)",
            custom => {
                // Custom numeric value
                window.txt_custom_fpp.set_text(custom);
                // Select "Custom..." option
                for (idx, opt) in window.combo_fpp.collection().iter().enumerate() {
                    if opt == "Custom..." {
                        window.combo_fpp.set_selection(Some(idx));
                        break;
                    }
                }
                window.txt_custom_fpp.set_visible(true);
                window.lbl_custom_fpp.set_visible(true);
                return Ok(());
            }
        };
        if let Some(index) = window.combo_fpp.collection().iter().position(|s| s == fpp_display) {
            window.combo_fpp.set_selection(Some(index));
        }
    }

    Ok(())
}

/// Save window values back to config.toml and return true if successful
pub fn save_config_from_window(window: &SettingsWindow) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_config_path();

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Load existing config or create new
    let mut config: toml::Value = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    // Update fields from window
    if let Some(table) = config.as_table_mut() {
        table.insert(
            "device_name".to_string(),
            toml::Value::String(window.txt_device_name.text()),
        );

        // Sample Rate
        if let Some(selected) = window.combo_sample_rate.selection() {
            if let Some(rate_str) = window.combo_sample_rate.collection().get(selected) {
                if let Ok(rate) = rate_str.parse::<i64>() {
                    table.insert("sample_rate".to_string(), toml::Value::Integer(rate));
                }
            }
        }

        // Channels
        if let Ok(channels) = window.txt_channels.text().parse::<i64>() {
            table.insert("channels".to_string(), toml::Value::Integer(channels));
        }

        // Latency (ms)
        if let Ok(latency) = window.txt_latency.text().parse::<i64>() {
            table.insert("latency_ms".to_string(), toml::Value::Integer(latency));
        }

        // Network Interface - extract name from "name (ip)" format
        if let Some(selected) = window.combo_network_interface.selection() {
            if let Some(nic_str) = window.combo_network_interface.collection().get(selected) {
                let nic_name = nic_str.split(" (").next().unwrap_or("").to_string();
                table.insert("network_interface".to_string(), toml::Value::String(nic_name));
            }
        }

        // FPP Mode
        if let Some(selected) = window.combo_fpp.selection() {
            if let Some(fpp_display) = window.combo_fpp.collection().get(selected) {
                let fpp_value = match fpp_display.as_str() {
                    "Auto (negotiate)" => "auto".to_string(),
                    "Min Latency (1 packet)" => "min".to_string(),
                    "Max Efficiency (64 packets)" => "max".to_string(),
                    "Custom..." => window.txt_custom_fpp.text(),
                    _ => "auto".to_string(),
                };
                table.insert("fpp".to_string(), toml::Value::String(fpp_value));
            }
        }
    }

    let toml_str = toml::to_string_pretty(&config)?;
    std::fs::write(&path, toml_str)?;

    Ok(())
}
