use std::process::Command;

pub struct FirewallResult {
    pub rules_added: Vec<String>,
    pub errors: Vec<String>,
}

/// Add all required Inferno firewall rules using netsh.
/// Returns list of rules added and any errors encountered.
pub fn setup_firewall_rules() -> FirewallResult {
    let rules: &[(&str, &str, &str)] = &[
        ("Inferno-Dante-RX-in",  "4440,4455", "in"),
        ("Inferno-Dante-RX-out", "4440,4455", "out"),
        ("Inferno-mDNS-in",      "5353",      "in"),
        ("Inferno-mDNS-out",     "5353",      "out"),
        ("Inferno-PTP-in",       "319,320",   "in"),
        ("Inferno-PTP-out",      "319,320",   "out"),
        ("Inferno-RTP-in",       "6000-6015", "in"),
        ("Inferno-RTP-out",      "6000-6015", "out"),
    ];
    
    let mut result = FirewallResult { rules_added: vec![], errors: vec![] };
    
    for (name, ports, dir) in rules {
        let output = Command::new("netsh")
            .args(["advfirewall", "firewall", "add", "rule",
                   &format!("name={}", name),
                   &format!("dir={}", dir),
                   "action=allow",
                   "protocol=UDP",
                   &format!("localport={}", ports)])
            .output();
        
        match output {
            Ok(o) if o.status.success() => {
                tracing::info!("Firewall rule added: {}", name);
                result.rules_added.push(name.to_string());
            }
            Ok(o) => {
                let err_msg = format!("{}: {}", name, String::from_utf8_lossy(&o.stderr));
                tracing::error!("Firewall rule failed (may require admin): {}", err_msg);
                result.errors.push(err_msg);
            }
            Err(e) => {
                let err_msg = format!("{}: {}", name, e);
                tracing::error!("Firewall rule failed: {}", err_msg);
                result.errors.push(err_msg);
            }
        }
    }
    result
}

/// Check if Inferno firewall rules already exist.
pub fn firewall_rules_exist() -> bool {
    Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", "name=Inferno-Dante-RX-in"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
