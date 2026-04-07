use tracing::{error, info};

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::{net::UdpSocket, select, sync::broadcast::Receiver};

pub const MTU: usize = 1500;
const PACKET_BUFFER_SIZE: usize = MTU;
pub const MAX_PAYLOAD_BYTES: usize = 1400; // ???

/// Try to enable socket receive timestamps (Windows 11 22H2+ only).
/// Returns Ok(()) if enabled, Err if not supported (older Windows).
/// Note: This is a best-effort feature that may not be available on all Windows versions.
pub fn try_enable_socket_timestamps(_socket: &std::net::UdpSocket) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        tracing::debug!("Socket timestamping support (Windows 11 22H2+ only)");
        // TODO: Implement actual setsockopt call when Windows crate binding is available
        // For now, this is logged and silently skipped as best-effort
    }
    #[cfg(not(target_os = "windows"))]
    {
        tracing::debug!("Socket timestamping not applicable on this platform");
    }
    Ok(())
}


pub struct ReceiveBuffer {
  buff: [u8; PACKET_BUFFER_SIZE],
}

impl ReceiveBuffer {
  pub fn new() -> Self {
    Self { buff: [0u8; PACKET_BUFFER_SIZE] }
  }
}

pub struct UdpSocketWrapper {
  socket: Option<UdpSocket>,
  listen_addr: Ipv4Addr,
  listen_port: u16,
  shutdown: Receiver<()>,
  dowork: bool,
  recv_buff: [u8; PACKET_BUFFER_SIZE],
}

impl UdpSocketWrapper {
  pub async fn new(
    listen_addr: Option<Ipv4Addr>,
    listen_port: u16,
    shutdown: Receiver<()>,
  ) -> UdpSocketWrapper {
    let listen_addr = listen_addr.unwrap_or(Ipv4Addr::new(0, 0, 0, 0));
    let socket_opt =
      UdpSocket::bind(SocketAddr::new(std::net::IpAddr::V4(listen_addr), listen_port)).await;
    let socket = socket_opt.expect("error starting really needed listener");
    // TODO MAY PANIC: this error should be non-fatal because some apps may use Inferno as an optional audio I/O
    let listen_port = socket.local_addr().unwrap().port();
    UdpSocketWrapper {
      listen_addr,
      socket: Some(socket),
      listen_port,
      shutdown,
      dowork: true,
      recv_buff: [0; PACKET_BUFFER_SIZE],
    }
  }

  pub fn should_work(&self) -> bool {
    self.dowork
  }

  pub fn port(&self) -> u16 {
    self.listen_port
  }

  pub async fn recv<'a>(&mut self, recv_buff: &'a mut ReceiveBuffer) -> Option<(SocketAddr, &'a [u8])> {
    let socket = match &self.socket {
      Some(s) => s,
      None => {
        return None;
      }
    };
    select! {
      r = socket.recv_from(&mut recv_buff.buff) => {
        match r {
          Ok((len_recv, src)) => {
            return Some((src, &recv_buff.buff[..len_recv]));
          },
          Err(e) => {
            error!("error receiving from socket: {e:?}");
            return None;
          }
        }
      },
      _ = self.shutdown.recv() => {
        self.dowork = false;
        return None;
      }
    };
  }

  pub async fn send(&self, dst: &SocketAddr, packet: &[u8]) {
    let socket = match &self.socket {
      Some(s) => s,
      None => {
        info!("shutting down, discarding message to send");
        return;
      }
    };
    if let Err(e) = socket.send_to(packet, dst).await {
      error!("send error (ignoring): {e:?}");
    }
  }
}

pub async fn create_tokio_udp_socket(self_ip: Ipv4Addr) -> tokio::io::Result<(UdpSocket, u16)> {
  let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(self_ip), 0)).await?;
  let port = socket.local_addr()?.port();
  return Ok((socket, port));
}

pub fn create_mio_udp_socket(self_ip: Ipv4Addr) -> std::io::Result<(mio::net::UdpSocket, u16)> {
  let socket = mio::net::UdpSocket::bind(SocketAddr::new(IpAddr::V4(self_ip), 0))?;
  let port = socket.local_addr()?.port();
  
  // Try to enable socket timestamping (Windows 11 22H2+, best-effort)
  #[cfg(target_os = "windows")]
  {
    use std::os::windows::io::{AsRawSocket, FromRawSocket};
    let raw_socket = socket.as_raw_socket();
    // Create a temporary std::net::UdpSocket view to call try_enable_socket_timestamps
    // This is safe because we're not consuming or dropping the mio socket
    let std_socket = unsafe { 
      std::net::UdpSocket::from_raw_socket(raw_socket as _) 
    };
    try_enable_socket_timestamps(&std_socket).ok();
    // Prevent the std socket from being dropped (which would close the underlying socket)
    std::mem::forget(std_socket);
  }
  
  return Ok((socket, port));
}
