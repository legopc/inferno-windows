use crate::device_info::DeviceId;

pub const PORT: u16 = 8800;

#[derive(Debug, binary_serde::BinarySerde, Default)]
pub struct DeviceAdvertisement {
  pub process_id: u16,
  pub factory_device_id: DeviceId,
  pub unknown1_1: u16,
  pub unknown2_0: u16,
  pub ip_address: [u8; 4],
  pub info_request_port: u16,
  pub unknown3_0: u16,
}

pub const REQUEST_DEVICE_ADVERTISEMENT: u16 = 0x1001;
