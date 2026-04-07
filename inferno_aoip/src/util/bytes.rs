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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_make_u16_basic() {
    assert_eq!(make_u16(0x12, 0x34), 0x1234u16);
    assert_eq!(make_u16(0x00, 0x00), 0u16);
    assert_eq!(make_u16(0xFF, 0xFF), 0xFFFFu16);
  }

  #[test]
  fn test_make_u16_boundary_values() {
    assert_eq!(make_u16(0x80, 0x00), 0x8000u16);
    assert_eq!(make_u16(0x00, 0x80), 0x0080u16);
    assert_eq!(make_u16(0x01, 0x02), 0x0102u16);
  }

  #[test]
  fn test_h_l_functions() {
    assert_eq!(H(0x1234), 0x12);
    assert_eq!(L(0x1234), 0x34);
    assert_eq!(H(0x0000), 0x00);
    assert_eq!(L(0x0000), 0x00);
    assert_eq!(H(0xFFFF), 0xFF);
    assert_eq!(L(0xFFFF), 0xFF);
  }

  #[test]
  fn test_h_l_roundtrip() {
    for v in [0u16, 1, 127, 255, 256, 1000, 32768, 0xFFFF] {
      assert_eq!(make_u16(H(v), L(v)), v, "roundtrip failed for {v}");
    }
  }

  #[test]
  fn test_write_str_to_buffer() {
    let mut buf = [0u8; 10];
    write_str_to_buffer(&mut buf, 0, 5, "hello");
    assert_eq!(&buf[0..5], b"hello");
    assert_eq!(&buf[5..], [0, 0, 0, 0, 0]);
  }

  #[test]
  fn test_write_str_to_buffer_truncate() {
    let mut buf = [0u8; 10];
    write_str_to_buffer(&mut buf, 0, 3, "hello");
    assert_eq!(&buf[0..3], b"hel");
  }

  #[test]
  fn test_write_str_to_buffer_with_offset() {
    let mut buf = [0xFF; 10];
    write_str_to_buffer(&mut buf, 2, 4, "test");
    assert_eq!(&buf[0..2], [0xFF, 0xFF]);
    assert_eq!(&buf[2..6], b"test");
    assert_eq!(&buf[6..], [0xFF, 0xFF, 0xFF, 0xFF]);
  }

  #[test]
  fn test_read_0term_str_from_buffer() {
    let buf = b"hello\0world";
    let s = read_0term_str_from_buffer(buf, 0).unwrap();
    assert_eq!(s, "hello");
  }

  #[test]
  fn test_read_0term_str_from_buffer_with_offset() {
    let buf = b"prefix\0hello\0world";
    let s = read_0term_str_from_buffer(buf, 7).unwrap();
    assert_eq!(s, "hello");
  }

  #[test]
  fn test_read_0term_str_out_of_bounds() {
    let buf = b"hello";
    let result = read_0term_str_from_buffer(buf, 10);
    assert!(result.is_err());
  }

  #[test]
  fn test_read_0term_str_no_null_terminator() {
    let buf = b"hello";
    let s = read_0term_str_from_buffer(buf, 0).unwrap();
    assert_eq!(s, "hello");
  }

  #[test]
  fn test_align_wpos() {
    let mut buf = ByteBuffer::new();
    buf.write_u8(1); // pos = 1
    align_wpos(&mut buf, 4);
    assert_eq!(buf.get_wpos() % 4, 0);
    assert_eq!(buf.get_wpos(), 4);
  }
}
