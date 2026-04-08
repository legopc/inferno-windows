use std::{
  net::Ipv4Addr,
  sync::{Arc, RwLock},
};

use netdev::mac::MacAddr;

#[derive(Clone)]
pub struct Channel {
  pub factory_name: String,
  pub friendly_name: Arc<RwLock<String>>, // Arc is needed only because of Clone requirement, TODO: fix the ALSA plugin
}

pub type DeviceId = [u8; 8];

#[derive(Clone)] // TODO: this shouldn't need to be clonable, fix the ALSA plugin
pub struct DeviceInfo {
  pub ip_address: Ipv4Addr,
  pub netmask: Ipv4Addr,
  pub gateway: Ipv4Addr,
  pub mac_address: MacAddr,
  pub link_speed: u16,

  pub board_name: String,
  pub manufacturer: String,
  pub model_name: String,
  pub model_number: String, // _000000000000000b
  pub factory_device_id: DeviceId,
  pub process_id: u16,
  pub vendor_string: String,
  pub friendly_hostname: String, // Dante 31-char limit enforced in inferno_wasapi::config::load()
  pub factory_hostname: String,  // Dante 31-char limit enforced in inferno_wasapi::config::load()

  pub rx_channels: Vec<Channel>,
  pub tx_channels: Vec<Channel>,
  pub bits_per_sample: u8,
  pub pcm_type: u8, // usually 0xe, in older devices 4
  pub latency_ns: usize,
  pub tx_latency_ns: u32,
  pub sample_rate: u32,

  pub arc_port: u16,
  pub cmc_port: u16,
  pub flows_control_port: u16,
  pub info_request_port: u16,
}

impl DeviceInfo {
  pub fn latency_samples(&self) -> usize {
    self.latency_ns * (self.sample_rate as usize) / 1_000_000_000
  }

  /// Verify that hostname fields comply with Dante's 31-character limit
  pub fn validate_hostnames(&self) {
    debug_assert!(self.friendly_hostname.len() <= 31, 
      "friendly_hostname exceeds 31-char Dante limit: {}", self.friendly_hostname);
    debug_assert!(self.factory_hostname.len() <= 31, 
      "factory_hostname exceeds 31-char Dante limit: {}", self.factory_hostname);
  }
}
