use std::cmp::min;
use std::error::Error;
use std::io;
use std::str;

use bytebuffer::ByteBuffer;

pub fn H(u: u16) -> u8 {
  return (u >> 8) as u8;
}
pub fn L(u: u16) -> u8 {
  return u as u8;
}

pub fn make_u16(h: u8, l: u8) -> u16 {
  return ((h as u16) << 8) | (l as u16);
}

pub fn write_str_to_buffer(buffer: &mut [u8], offset: usize, max_len: usize, s: &str) {
  let len = min(max_len, s.len());
  buffer[offset..offset + len].clone_from_slice(&s.as_bytes()[0..len]);
}

pub fn write_0term_str_to_bytebuffer(bytes: &mut ByteBuffer, s: &str) -> u16 {
  let offset = bytes.get_wpos();
  bytes.write_bytes(s.as_bytes());
  bytes.write_u8(0);
  return offset.try_into().unwrap();
}

pub fn write_0term_str_or_0_to_bytebuffer(bytes: &mut ByteBuffer, s: Option<&str>) -> u16 {
  if let Some(s) = s {
    write_0term_str_to_bytebuffer(bytes, s)
  } else {
    0
  }
}

pub fn align_wpos(bytes: &mut ByteBuffer, alignment: usize) {
  while (bytes.get_wpos() % alignment) != 0 {
    bytes.write_u8(0);
  }
}

pub fn read_0term_str_from_buffer(buffer: &[u8], offset: usize) -> Result<&str, Box<dyn Error>> {
  if offset >= buffer.len() {
    return Err(Box::new(io::Error::from(io::ErrorKind::UnexpectedEof)));
  }
  let ntpos = match buffer[offset..].iter().position(|c| *c == 0) {
    Some(x) => x,
    None => buffer.len(),
  };
  return str::from_utf8(&buffer[offset..][..ntpos]).map_err(|e| Box::new(e) as Box<dyn Error>);
}
