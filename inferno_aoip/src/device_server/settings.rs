use std::{
  collections::BTreeMap,
  env,
  net::{IpAddr, Ipv4Addr},
  path::PathBuf,
  sync::{Arc, RwLock},
};

use netdev::mac::MacAddr;
use tracing::{debug, error, info};

use crate::protocol::flows_control::PORT as FLOWS_CONTROL_PORT;
use crate::protocol::mcast::INFO_REQUEST_PORT;
use crate::protocol::proto_arc::PORT as ARC_PORT;
use crate::protocol::proto_cmc::PORT as CMC_PORT;
use crate::device_info::{Channel, DeviceInfo};

fn model_number() -> String {
  std::env::var("DANTE_MODEL_NUMBER")
    .ok()
    .and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
    .map(|num| format!("_{:014x}b", num))
    .unwrap_or_else(|| "_000000000000000b".to_owned())
}

// Named constants for configuration defaults
const DEFAULT_BIND_IP_PARSE_ERR: &str = "BIND_IP: must contain IP address or network interface name";
const DEFAULT_PROCESS_ID: u16 = 0;
const DEFAULT_RX_LATENCY_NS: usize = 10_000_000;
const DEFAULT_TX_LATENCY_NS: u32 = 10_000_000;
const DEFAULT_SAMPLE_RATE: u32 = 48000;
const HOSTNAME_MAX_LEN: usize = 31;
const DEFAULT_USE_SAFE_CLOCK: bool = false;
const MAX_SHORT_APPNAME_LEN: usize = 14;
const MAX_APPNAME_FOR_HOSTNAME: usize = 22;

fn create_self_info(
  app_name: &str,
  short_app_name: &str,
  my_ip: Option<Ipv4Addr>,
  settings: &BTreeMap<String, String>,
) -> DeviceInfo {
  // Configuration errors are logged but not fatal; we use sensible defaults
  // This allows ALSA plugins and other embedded use cases to continue with degraded functionality

  let interfaces = netdev::get_interfaces();
  let my_ipv4 = my_ip
    .or_else(|| {
      settings.get("BIND_IP").and_then(|ipstr| {
        match ipstr.parse::<Ipv4Addr>() {
          Ok(ip) => Some(ip),
          Err(_) => {
            // Try to find interface by name
            interfaces.iter().find(|iface| &iface.name == ipstr)
              .and_then(|iface| iface.ipv4.get(0).map(|ipv4| ipv4.addr()))
              .or_else(|| {
                error!("BIND_IP setting invalid: '{}', must be IP address or interface name", ipstr);
                None
              })
          }
        }
      })
    })
    .or_else(|| {
      match local_ip_address::local_ip() {
        Ok(IpAddr::V4(a)) => {
          info!(local_ip = %a, "using discovered local IPv4");
          Some(a)
        },
        Ok(IpAddr::V6(_)) => {
          error!("discovered local IP is IPv6, only IPv4 supported");
          None
        },
        Err(e) => {
          error!("failed to discover local IP: {}", e);
          None
        }
      }
    })
    .unwrap_or_else(|| Ipv4Addr::LOCALHOST);

  let process_id: u16 = settings
    .get("PROCESS_ID")
    .and_then(|s| match s.parse::<u16>() {
      Ok(id) => Some(id),
      Err(e) => {
        error!("PROCESS_ID parsing failed: '{}' - {}, using default {}", s, e, DEFAULT_PROCESS_ID);
        None
      }
    })
    .unwrap_or(DEFAULT_PROCESS_ID);

  let mut devid = [0u8; 8];
  if let Some(idstr) = settings.get("DEVICE_ID") {
    match hex::decode_to_slice(idstr, &mut devid) {
      Ok(_) => debug!("using DEVICE_ID from config: {}", idstr),
      Err(e) => {
        error!("DEVICE_ID parsing failed: '{}' - {}, generating from IP", idstr, e);
        devid[2..6].copy_from_slice(&my_ipv4.octets());
        devid[6..8].copy_from_slice(&process_id.to_be_bytes());
      }
    }
  } else {
    devid[2..6].copy_from_slice(&my_ipv4.octets());
    devid[6..8].copy_from_slice(&process_id.to_be_bytes());
  }

  // TODO make hostname and sample rate configurable from DC
  let friendly_hostname = settings
    .get("NAME")
    .map(|s| if s.len() > HOSTNAME_MAX_LEN { s[0..HOSTNAME_MAX_LEN].to_owned() } else { s.clone() })
    .unwrap_or_else(|| {
      let app_part = if app_name.len() > MAX_APPNAME_FOR_HOSTNAME { &app_name[0..MAX_APPNAME_FOR_HOSTNAME] } else { &app_name };
      format!(
        "{} {}",
        app_part,
        hex::encode(&my_ipv4.octets())
      )
    });
  let short_app_name = if short_app_name.len() > MAX_SHORT_APPNAME_LEN { &short_app_name[0..MAX_SHORT_APPNAME_LEN] } else { short_app_name };

  let sample_rate = settings
    .get("SAMPLE_RATE")
    .and_then(|s| match s.parse::<u32>() {
      Ok(rate) => Some(rate),
      Err(e) => {
        error!("SAMPLE_RATE parsing failed: '{}' - {}, using default {}", s, e, DEFAULT_SAMPLE_RATE);
        None
      }
    })
    .unwrap_or(DEFAULT_SAMPLE_RATE);

  let mut netmask = Ipv4Addr::new(0, 0, 0, 0);
  let mut gateway = Ipv4Addr::new(0, 0, 0, 0);
  let mut mac_address = MacAddr::zero();
  let mut speed = 0;
  for iface in interfaces {
    let mut our_iface = false;
    for network in iface.ipv4 {
      if network.addr() == my_ipv4 {
        netmask = network.netmask();
        our_iface = true;
        break;
      }
    }
    if our_iface {
      speed =
        [iface.transmit_speed.unwrap_or(0), iface.receive_speed.unwrap_or(0)].iter().max().unwrap_or(&0)
          / 1_000_000;
      if let Some(gws) = iface.gateway {
        for gw in gws.ipv4 {
          if (gw.to_bits() & netmask.to_bits()) == (my_ipv4.to_bits() & netmask.to_bits()) {
            gateway = gw;
            break;
          }
        }
      }
      if let Some(mac) = iface.mac_addr {
        mac_address = mac;
      }
      break;
    }
  }

  let latency_ns = settings
    .get("RX_LATENCY_NS")
    .and_then(|s| match s.parse::<usize>() {
      Ok(ns) => Some(ns),
      Err(e) => {
        error!("RX_LATENCY_NS parsing failed: '{}' - {}, using default {}", s, e, DEFAULT_RX_LATENCY_NS);
        None
      }
    })
    .unwrap_or(DEFAULT_RX_LATENCY_NS);

  let tx_latency_ns = settings
    .get("TX_LATENCY_NS")
    .and_then(|s| match s.parse::<u32>() {
      Ok(ns) => Some(ns),
      Err(e) => {
        error!("TX_LATENCY_NS parsing failed: '{}' - {}, using default {}", s, e, DEFAULT_TX_LATENCY_NS);
        None
      }
    })
    .unwrap_or(DEFAULT_TX_LATENCY_NS);

  let link_speed_clamped = speed.clamp(0, 10000) as u16;

  let mut result = DeviceInfo {
    ip_address: my_ipv4,
    netmask,
    gateway,
    mac_address,
    link_speed: link_speed_clamped,
    
    board_name: "Inferno-AoIP".to_owned(),
    manufacturer: "Inferno-AoIP".to_owned(),
    model_name: app_name.to_owned(),
    factory_device_id: devid,
    process_id,
    vendor_string: "Audinate Dante-compatible".to_owned(),
    factory_hostname: format!("{short_app_name}-{}", hex::encode(devid)),
    friendly_hostname,
    model_number: model_number(),
    rx_channels: vec![],
    tx_channels: vec![],
    bits_per_sample: 24, // TODO make it configurable
    pcm_type: 0xe,
    latency_ns,
    tx_latency_ns,
    sample_rate,

    arc_port: ARC_PORT,
    cmc_port: CMC_PORT,
    flows_control_port: FLOWS_CONTROL_PORT,
    info_request_port: INFO_REQUEST_PORT,
  };

  if let Some(altport_str) = settings.get("ALT_PORT") {
    match altport_str.parse::<u16>() {
      Ok(altport) => {
        info!(alt_port = altport, "using alternate port base");
        result.arc_port = altport;
        result.cmc_port = altport + 1;
        result.flows_control_port = altport + 2;
        result.info_request_port = altport + 3;
      },
      Err(e) => {
        error!("ALT_PORT parsing failed: '{}' - {}, using default ports", altport_str, e);
      }
    }
  }

  info!(
    ip = %result.ip_address,
    device_id = %hex::encode(result.factory_device_id),
    process_id = result.process_id,
    friendly_hostname = %result.friendly_hostname,
    sample_rate = result.sample_rate,
    latency_ns = result.latency_ns,
    "device configuration initialized"
  );

  result
}

#[derive(Clone)] // TODO: this shouldn't need to be clonable, fix the ALSA plugin
pub struct Settings {
  pub self_info: DeviceInfo,
  pub tx_latency_ns: u32,
  pub clock_path: Option<PathBuf>,
  pub use_safe_clock: bool,
  /// Frames per packet override: None=negotiate (auto), Some(n)=force specific value
  pub fpp_override: Option<u32>,
  /// Ring buffer size in samples per channel (must be power of 2, default 524288)
  pub rx_buffer_samples: usize,
  /// Latency reference in samples (default 4800 = 100ms at 48kHz)
  pub latency_ref_samples: usize,
}

impl Settings {
  pub fn new(
    app_name: &str,
    short_app_name: &str,
    my_ip: Option<Ipv4Addr>,
    config: &BTreeMap<String, String>,
  ) -> Self {
    // convert all settings keys to upper case:
    let mut config: BTreeMap<String, String> =
      config.clone().into_iter().map(|(k, v)| (k.to_ascii_uppercase(), v)).collect();

    // add settings from env vars if not already set:
    env::vars().for_each(|(env_key, env_value)| {
      if let Some(key) = env_key.strip_prefix("INFERNO_") {
        let key = key.to_ascii_uppercase();
        config.entry(key).or_insert(env_value);
      }
    });
    let self_info = create_self_info(app_name, short_app_name, my_ip, &config);

    let use_safe_clock = config
      .get("USE_SAFE_CLOCK")
      .and_then(|s| match s.parse::<bool>() {
        Ok(b) => Some(b),
        Err(e) => {
          error!("USE_SAFE_CLOCK parsing failed: '{}' - {}, using default {}", s, e, DEFAULT_USE_SAFE_CLOCK);
          None
        }
      })
      .unwrap_or(DEFAULT_USE_SAFE_CLOCK);

    let tx_latency_ns = config
      .get("TX_LATENCY_NS")
      .and_then(|p| match p.parse::<u32>() {
        Ok(ns) => Some(ns),
        Err(e) => {
          error!("TX_LATENCY_NS parsing failed: '{}' - {}, using default {}", p, e, DEFAULT_TX_LATENCY_NS);
          None
        }
      })
      .unwrap_or(DEFAULT_TX_LATENCY_NS);

    let clock_path = config.get("CLOCK_PATH").and_then(|p| {
      match std::str::FromStr::from_str(p) {
        Ok(path) => Some(path),
        Err(e) => {
          error!("CLOCK_PATH parsing failed: '{}' - {}", p, e);
          None
        }
      }
    });

    let mut result = Self {
      self_info,
      tx_latency_ns,
      clock_path,
      use_safe_clock,
      fpp_override: None,
      rx_buffer_samples: 524288,
      latency_ref_samples: 4800,
    };

    // the following should be harmless, as the application still has the chance to overwrite it
    let rx_count = config
      .get("RX_CHANNELS")
      .and_then(|s| match s.parse::<u16>() {
        Ok(count) => Some(count as usize),
        Err(e) => {
          error!("RX_CHANNELS parsing failed: '{}' - {}, using default 2", s, e);
          None
        }
      })
      .unwrap_or(2);
    result.make_rx_channels(rx_count);

    let tx_count = config
      .get("TX_CHANNELS")
      .and_then(|s| match s.parse::<u16>() {
        Ok(count) => Some(count as usize),
        Err(e) => {
          error!("TX_CHANNELS parsing failed: '{}' - {}, using default 2", s, e);
          None
        }
      })
      .unwrap_or(2);
    result.make_tx_channels(tx_count);

    info!(
      rx_channels = rx_count,
      tx_channels = tx_count,
      clock_path = ?result.clock_path,
      use_safe_clock = result.use_safe_clock,
      "settings initialized"
    );

    result
  }
  pub fn make_rx_channels(&mut self, count: usize) {
    self.self_info.rx_channels = (1..=count)
      .map(|id| Channel {
        factory_name: format!("{id:02}"),
        friendly_name: Arc::new(RwLock::new(format!("RX {id}"))),
      })
      .collect();
    info!(channels = count, "RX channels configured");
  }
  pub fn make_tx_channels(&mut self, count: usize) {
    self.self_info.tx_channels = (1..=count)
      .map(|id| Channel {
        factory_name: format!("{id:02}"),
        friendly_name: Arc::new(RwLock::new(format!("TX {id}"))),
      })
      .collect();
    info!(channels = count, "TX channels configured");
  }
}
