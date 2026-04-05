use binary_layout::prelude::*;

use crate::byte_utils::*;

pub const HEADER_LENGTH: usize = 32;
pub const INFO_REQUEST_PORT: u16 = 8700;

define_layout!(mcast_packet, BigEndian, {
  start_code: u16,
  total_length: u16,
  seqnum: u16,
  process: u16,
  factory_device_id: [u8; 8],
  vendor: [u8; 8],
  opcode: [u8; 8],
  content: [u8]
});

pub fn make_packet<'a>(
  buffer: &'a mut [u8],
  start_code: u16,
  seqnum: u16,
  process: u16,
  factory_device_id: [u8; 8],
  vendor_str: [u8; 8],
  opcode: [u8; 8],
  content: &[u8],
) -> &'a [u8] {
  let total_len = content.len() + HEADER_LENGTH;
  assert!(total_len <= (1 << 16)); // TODO MAY PANIC
  let buffer = &mut buffer[..total_len]; // TODO MAY PANIC check length before slicing
  let mut view = mcast_packet::View::new(buffer);
  view.start_code_mut().write(start_code);
  view.total_length_mut().write(total_len as u16);
  view.seqnum_mut().write(seqnum);
  view.process_mut().write(process);
  view.factory_device_id_mut().copy_from_slice(&factory_device_id);
  view.vendor_mut().copy_from_slice(&vendor_str);
  view.opcode_mut().copy_from_slice(&opcode);
  view.content_mut().copy_from_slice(&content);
  return view.into_storage();
}


pub struct MulticastMessage {
  pub start_code: u16,
  pub opcode: [u8; 8],
  pub content: Vec<u8>,
}

pub fn make_channel_change_notification(
  channel_indices: impl IntoIterator<Item = usize>,
) -> MulticastMessage {
  let mut content = vec![0u8; 3];
  let offset = 2;
  for ch in channel_indices {
    let byte = ch / 8;
    let bit = ch % 8;
    if byte >= (content.len() - offset) {
      content.resize(byte + offset + 1, 0);
    }
    content[byte + offset] |= 1 << bit;
  }
  let mask_len = (content.len() - 2).try_into().unwrap();
  content[0] = H(mask_len);
  content[1] = L(mask_len);
  MulticastMessage { start_code: 0xffff, opcode: [0x07, 0x2a, 1, 2, 0, 0, 0, 0], content }
}
