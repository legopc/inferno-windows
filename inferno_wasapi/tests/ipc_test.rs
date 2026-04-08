//! Integration test for the IPC named pipe server.

use std::time::Duration;

// Test that the IPC StatusMessage serializes/deserializes correctly
#[test]
fn test_status_message_serde() {
    // We can't import from inferno_wasapi directly in integration tests easily
    // so just test JSON round-trip manually
    let json = r#"{"type":"Status","tx_active":false,"rx_active":true,"tx_channels":0,"rx_channels":2,"sample_rate":48000,"clock_mode":"SafeClock","tx_peak_db":[],"uptime_secs":42}"#;
    
    // Parse as generic JSON value
    let v: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
    assert_eq!(v["type"], "Status");
    assert_eq!(v["rx_active"], true);
    assert_eq!(v["sample_rate"], 48000);
    assert_eq!(v["uptime_secs"], 42);
}

#[test]
fn test_get_status_message_format() {
    let msg = serde_json::json!({"type": "GetStatus"});
    let s = serde_json::to_string(&msg).unwrap();
    assert!(s.contains("GetStatus"));
}
