use native_windows_gui as nwg;
use std::path::PathBuf;

/// First-run wizard for initial setup
/// Guides user through NIC selection, firewall rules, and config creation
#[allow(dead_code)]
#[derive(Default)]
pub struct FirstRunWizard {
    pub window: nwg::Window,
    pub lbl_title: nwg::Label,
    pub lbl_body: nwg::Label,

    // Step 1 - NIC Selection
    pub lbl_nic: nwg::Label,
    pub combo_nic: nwg::ComboBox<String>,

    // Step 2 - Firewall Rules
    pub lbl_firewall: nwg::Label,
    pub btn_apply_firewall: nwg::Button,
    pub lbl_firewall_status: nwg::Label,

    // Step 3 - Ready
    pub lbl_summary: nwg::Label,

    // Navigation
    pub btn_next: nwg::Button,
    pub btn_back: nwg::Button,
    pub btn_finish: nwg::Button,
    pub lbl_step: nwg::Label,

    // Internal state
    pub current_step: std::sync::Arc<std::cell::Cell<u32>>,
    pub selected_nic: std::sync::Arc<std::cell::Cell<Option<String>>>,
    pub firewall_applied: std::sync::Arc<std::cell::Cell<bool>>,
}

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

pub fn get_config_path() -> PathBuf {
    if let Some(local_data_dir) = dirs::data_local_dir() {
        local_data_dir.join("inferno_aoip").join("config.toml")
    } else {
        PathBuf::from("config.toml")
    }
}

pub fn apply_firewall_rules() -> Result<String, String> {
    use std::process::Command;

    // Rule 1: Dante RX ports (UDP 4440, 4455, 5353, 6000-6015)
    let rule1 = Command::new("netsh")
        .args(&[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            "name=Inferno Dante RX",
            "dir=in",
            "action=allow",
            "protocol=UDP",
            "localport=4440,4455,5353,6000-6015",
        ])
        .output();

    // Rule 2: PTP ports (UDP 319, 320)
    let rule2 = Command::new("netsh")
        .args(&[
            "advfirewall",
            "firewall",
            "add",
            "rule",
            "name=Inferno PTP",
            "dir=in",
            "action=allow",
            "protocol=UDP",
            "localport=319,320",
        ])
        .output();

    match (rule1, rule2) {
        (Ok(out1), Ok(out2)) if out1.status.success() && out2.status.success() => {
            Ok("Firewall rules applied successfully.".to_string())
        }
        _ => {
            Err("Firewall rules require admin rights — run as Administrator or add rules manually.".to_string())
        }
    }
}

pub fn write_config(nic: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_config_path();

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create minimal config
    let mut config = toml::map::Map::new();
    config.insert(
        "device_name".to_string(),
        toml::Value::String("Inferno".to_string()),
    );
    config.insert(
        "network_interface".to_string(),
        toml::Value::String(nic.to_string()),
    );
    config.insert("sample_rate".to_string(), toml::Value::Integer(48000));
    config.insert("channels".to_string(), toml::Value::Integer(2));
    config.insert("latency_ms".to_string(), toml::Value::Integer(10));
    config.insert(
        "fpp".to_string(),
        toml::Value::String("auto".to_string()),
    );

    let config_value = toml::Value::Table(config);
    let toml_str = toml::to_string_pretty(&config_value)?;
    std::fs::write(&path, toml_str)?;

    Ok(())
}

pub fn show_wizard() -> Result<(), Box<dyn std::error::Error>> {
    nwg::init()?;
    nwg::Font::set_global_family("Segoe UI")?;

    let mut window: nwg::Window = Default::default();
    nwg::Window::builder()
        .size((450, 350))
        .position((300, 300))
        .title("Inferno - First Run Setup")
        .build(&mut window)?;

    let mut lbl_title: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("First Run Setup")
        .position((10, 15))
        .size((430, 25))
        .parent(&window)
        .build(&mut lbl_title)?;

    let mut lbl_body: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("")
        .position((10, 50))
        .size((430, 200))
        .parent(&window)
        .build(&mut lbl_body)?;

    // Step 1 - NIC Selection
    let mut lbl_nic: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Select Network Interface:")
        .position((10, 50))
        .size((430, 20))
        .parent(&window)
        .build(&mut lbl_nic)?;

    let nics = list_network_interfaces();
    let mut combo_nic: nwg::ComboBox<String> = Default::default();
    nwg::ComboBox::builder()
        .collection(nics)
        .parent(&window)
        .size((430, 25))
        .position((10, 75))
        .build(&mut combo_nic)?;

    // Step 2 - Firewall Rules
    let mut lbl_firewall: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Configure Firewall Rules\n\nPorts: 4440, 4455, 5353, 319, 320, 6000-6015")
        .position((10, 50))
        .size((430, 100))
        .parent(&window)
        .build(&mut lbl_firewall)?;

    let mut btn_apply_firewall: nwg::Button = Default::default();
    nwg::Button::builder()
        .text("Apply Firewall Rules")
        .position((10, 160))
        .size((430, 32))
        .parent(&window)
        .build(&mut btn_apply_firewall)?;

    let mut lbl_firewall_status: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("")
        .position((10, 200))
        .size((430, 50))
        .parent(&window)
        .build(&mut lbl_firewall_status)?;

    // Step 3 - Ready
    let mut lbl_summary: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("")
        .position((10, 50))
        .size((430, 150))
        .parent(&window)
        .build(&mut lbl_summary)?;

    // Navigation buttons
    let mut btn_next: nwg::Button = Default::default();
    nwg::Button::builder()
        .text("Next")
        .position((270, 265))
        .size((80, 32))
        .parent(&window)
        .build(&mut btn_next)?;

    let mut btn_back: nwg::Button = Default::default();
    nwg::Button::builder()
        .text("Back")
        .position((180, 265))
        .size((80, 32))
        .parent(&window)
        .build(&mut btn_back)?;

    let mut btn_finish: nwg::Button = Default::default();
    nwg::Button::builder()
        .text("Finish")
        .position((270, 265))
        .size((80, 32))
        .parent(&window)
        .build(&mut btn_finish)?;

    let mut lbl_step: nwg::Label = Default::default();
    nwg::Label::builder()
        .text("Step 1 of 3")
        .position((10, 270))
        .size((150, 25))
        .parent(&window)
        .build(&mut lbl_step)?;

    // State
    let current_step = std::sync::Arc::new(std::cell::Cell::new(1u32));
    let selected_nic = std::sync::Arc::new(std::cell::Cell::new(None::<String>));
    let firewall_applied = std::sync::Arc::new(std::cell::Cell::new(false));

    // Initially show step 1
    update_wizard_ui(
        &lbl_title,
        &lbl_body,
        &lbl_nic,
        &combo_nic,
        &lbl_firewall,
        &btn_apply_firewall,
        &lbl_firewall_status,
        &lbl_summary,
        &btn_back,
        &btn_finish,
        &lbl_step,
        current_step.clone(),
    );

    // Event handler
    let window_handle = window.handle;
    let btn_next_handle = btn_next.handle;
    let btn_back_handle = btn_back.handle;
    let btn_finish_handle = btn_finish.handle;
    let btn_apply_firewall_handle = btn_apply_firewall.handle;

    let step_clone = current_step.clone();
    let nic_clone = selected_nic.clone();
    let firewall_clone = firewall_applied.clone();
    let combo_nic_handle = combo_nic.handle;
    let lbl_firewall_status_handle = lbl_firewall_status.handle;

    let step_clone2 = current_step.clone();
    let nic_clone2 = selected_nic.clone();
    let firewall_clone2 = firewall_applied.clone();

    let should_finish = std::sync::Arc::new(std::cell::Cell::new(false));
    let should_finish_clone = should_finish.clone();

    let handler = nwg::full_bind_event_handler(
        &window_handle,
        move |evt, _data, handle| {
            use nwg::Event as E;
            let step = step_clone.get();

            match evt {
                E::OnButtonClick => {
                    if handle == btn_next_handle && step < 3 {
                        // Save NIC selection on step 1
                        if step == 1 {
                            if let Some(sel) = combo_nic.selection() {
                                if let Some(nic_str) = combo_nic.collection().get(sel) {
                                    let nic_name = nic_str.split(" (").next().unwrap_or("").to_string();
                                    nic_clone.set(Some(nic_name));
                                }
                            }
                        }
                        step_clone.set(step + 1);
                        update_wizard_ui(
                            &lbl_title,
                            &lbl_body,
                            &lbl_nic,
                            &combo_nic,
                            &lbl_firewall,
                            &btn_apply_firewall,
                            &lbl_firewall_status,
                            &lbl_summary,
                            &btn_back,
                            &btn_finish,
                            &lbl_step,
                            step_clone.clone(),
                        );
                    } else if handle == btn_back_handle && step > 1 {
                        step_clone.set(step - 1);
                        update_wizard_ui(
                            &lbl_title,
                            &lbl_body,
                            &lbl_nic,
                            &combo_nic,
                            &lbl_firewall,
                            &btn_apply_firewall,
                            &lbl_firewall_status,
                            &lbl_summary,
                            &btn_back,
                            &btn_finish,
                            &lbl_step,
                            step_clone.clone(),
                        );
                    } else if handle == btn_apply_firewall_handle && step == 2 {
                        match apply_firewall_rules() {
                            Ok(msg) => {
                                lbl_firewall_status.set_text(&format!("✓ {}", msg));
                                firewall_clone.set(true);
                            }
                            Err(msg) => {
                                lbl_firewall_status.set_text(&format!("✗ {}", msg));
                            }
                        }
                    } else if handle == btn_finish_handle && step == 3 {
                        should_finish_clone.set(true);
                        nwg::stop_thread_dispatch();
                    }
                }
                E::OnWindowClose => {
                    nwg::stop_thread_dispatch();
                }
                _ => {}
            }
        },
    );

    // Show step 1 initially
    update_wizard_ui(
        &lbl_title,
        &lbl_body,
        &lbl_nic,
        &combo_nic,
        &lbl_firewall,
        &btn_apply_firewall,
        &lbl_firewall_status,
        &lbl_summary,
        &btn_back,
        &btn_finish,
        &lbl_step,
        step_clone2.clone(),
    );

    // Dispatch events
    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);

    // After closing, if user finished, write config
    if should_finish.get() {
        if let Some(nic) = nic_clone2.get() {
            write_config(&nic)?;
        }
    }

    Ok(())
}

fn update_wizard_ui(
    lbl_title: &nwg::Label,
    lbl_body: &nwg::Label,
    lbl_nic: &nwg::Label,
    combo_nic: &nwg::ComboBox<String>,
    lbl_firewall: &nwg::Label,
    btn_apply_firewall: &nwg::Button,
    lbl_firewall_status: &nwg::Label,
    lbl_summary: &nwg::Label,
    btn_back: &nwg::Button,
    btn_finish: &nwg::Button,
    lbl_step: &nwg::Label,
    current_step: std::sync::Arc<std::cell::Cell<u32>>,
) {
    let step = current_step.get();

    // Update step indicator
    lbl_step.set_text(&format!("Step {} of 3", step));

    // Hide all step-specific controls
    lbl_nic.set_visible(false);
    combo_nic.set_visible(false);
    lbl_firewall.set_visible(false);
    btn_apply_firewall.set_visible(false);
    lbl_firewall_status.set_visible(false);
    lbl_summary.set_visible(false);

    // Show/hide back/next buttons
    btn_back.set_visible(step > 1);
    btn_finish.set_visible(step == 3);

    match step {
        1 => {
            lbl_title.set_text("Step 1: Select Network Interface");
            lbl_body.set_text(
                "Choose the Dante network interface for Inferno to use:",
            );
            lbl_nic.set_visible(true);
            combo_nic.set_visible(true);
        }
        2 => {
            lbl_title.set_text("Step 2: Configure Firewall");
            lbl_body.set_text(
                "Allow Inferno through the Windows Firewall.\nClick 'Apply Firewall Rules' to add necessary ports.",
            );
            lbl_firewall.set_visible(true);
            btn_apply_firewall.set_visible(true);
            lbl_firewall_status.set_visible(true);
        }
        3 => {
            lbl_title.set_text("Step 3: Ready to Start");
            lbl_body.set_text(
                "Configuration complete! Click 'Finish' to save and start Inferno.",
            );
            lbl_summary.set_visible(true);
        }
        _ => {}
    }
}
