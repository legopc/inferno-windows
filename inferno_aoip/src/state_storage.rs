use std::{
  error::Error,
  fs::{create_dir_all, File},
  io::{Read, Write},
  path::MAIN_SEPARATOR_STR,
  sync::Arc,
  net::Ipv4Addr,
};

use crate::{common::*, device_info, device_info::DeviceInfo};
use platform_dirs::AppDirs;
use serde::{Deserialize, Serialize};
use toml;

const PATH_SUFFIX: &str = ".toml";

/// Saved state for a single TX flow
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedTxFlow {
  pub flow_id: u32,
  pub channel_indices: Vec<Option<usize>>,
  pub multicast_addr: Option<String>,
  pub multicast_port: u16,
}

/// Saved state for all TX flows
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedTxFlows {
  pub flows: Vec<SavedTxFlow>,
}

pub struct StateStorage {
  path_prefix: String,
}

impl StateStorage {
  pub fn new(self_info: &DeviceInfo) -> Self {
    let dir = AppDirs::new(Some("inferno_aoip"), false).unwrap().state_dir.to_str().unwrap().to_owned()
      + MAIN_SEPARATOR_STR
      + &hex::encode(self_info.factory_device_id);
    create_dir_all(&dir).log_and_forget();
    info!("using state directory: {dir}");
    Self { path_prefix: dir + MAIN_SEPARATOR_STR }
  }
  fn full_path(&self, name: &str) -> String {
    format!("{}{name}{PATH_SUFFIX}", self.path_prefix)
  }
  pub fn save(&self, name: &str, value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    let content = toml::to_string(&value)?;
    let tmp_path = self.full_path(&format!("tmp.{name}"));
    let mut file = File::create(&tmp_path)?;
    file.write(content.as_bytes())?;
    drop(file);
    std::fs::rename(tmp_path, self.full_path(name))?;
    Ok(())
  }
  pub fn load<T: for<'a> Deserialize<'a>>(&self, name: &str) -> Result<T, Box<dyn Error>> {
    let mut file = File::open(self.full_path(name))?;
    let mut content: String = "".to_owned();
    file.read_to_string(&mut content)?;
    Ok(toml::from_str(&content)?)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_saved_tx_flow_default() {
    let flow = SavedTxFlow::default();
    assert_eq!(flow.flow_id, 0);
    assert!(flow.channel_indices.is_empty());
    assert!(flow.multicast_addr.is_none());
    assert_eq!(flow.multicast_port, 0);
  }

  #[test]
  fn test_saved_tx_flows_default() {
    let flows = SavedTxFlows::default();
    assert!(flows.flows.is_empty());
  }

  #[test]
  fn test_saved_tx_flow_serialization() {
    let flow = SavedTxFlow {
      flow_id: 123,
      channel_indices: vec![Some(0), Some(1), None],
      multicast_addr: Some("224.0.0.1".to_string()),
      multicast_port: 5004,
    };

    // Test serialization of flow without None values (TOML limitation)
    let flow2 = SavedTxFlow {
      flow_id: 456,
      channel_indices: vec![Some(2), Some(3)],
      multicast_addr: Some("224.0.0.2".to_string()),
      multicast_port: 5005,
    };

    let toml_str = toml::to_string(&flow2).expect("serialization failed");
    let deserialized: SavedTxFlow = toml::from_str(&toml_str).expect("deserialization failed");
    
    assert_eq!(deserialized.flow_id, 456);
    assert_eq!(deserialized.channel_indices.len(), 2);
    assert_eq!(deserialized.multicast_addr, Some("224.0.0.2".to_string()));
    assert_eq!(deserialized.multicast_port, 5005);
  }

  #[test]
  fn test_saved_tx_flows_serialization() {
    let flows = SavedTxFlows {
      flows: vec![
        SavedTxFlow {
          flow_id: 1,
          channel_indices: vec![Some(0)],
          multicast_addr: Some("224.0.0.1".to_string()),
          multicast_port: 5004,
        },
        SavedTxFlow {
          flow_id: 2,
          channel_indices: vec![Some(1), Some(2)],
          multicast_addr: Some("224.0.0.2".to_string()),
          multicast_port: 5005,
        },
      ],
    };

    let toml_str = toml::to_string(&flows).expect("serialization failed");
    let deserialized: SavedTxFlows = toml::from_str(&toml_str).expect("deserialization failed");
    
    assert_eq!(deserialized.flows.len(), 2);
    assert_eq!(deserialized.flows[0].flow_id, 1);
    assert_eq!(deserialized.flows[1].flow_id, 2);
  }
}
