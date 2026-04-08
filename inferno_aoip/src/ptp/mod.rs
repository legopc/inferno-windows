// PTPv1 Sync message listener module
// Binds to multicast 224.0.1.129:319 and extracts clock synchronization data

use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::watch;
use log::{info, warn, debug};

/// Measured offset from a single PTP sync message
#[derive(Debug, Clone)]
pub struct PtpOffset {
    pub grandmaster_id: [u8; 6],
    pub sequence_id: u16,
    pub offset_ns: i64,   // local_recv_ns - origin_timestamp_ns
    pub recv_time: std::time::Instant,
}

/// PTP synchronization state
#[derive(Debug, Clone, PartialEq)]
pub enum PtpState {
    /// Synchronized with active PTP grandmaster
    Synced { grandmaster_id: [u8; 6] },
    /// Grace period after losing grandmaster, transitioning to free-running
    FallingBack,
    /// Free-running with SafeClock (no PTP discipline)
    FreRunning,
}

const PTP_MULTICAST: Ipv4Addr = Ipv4Addr::new(224, 0, 1, 129);
const PTP_PORT: u16 = 319;
const PTP_MESSAGE_SIZE: usize = 124;

// PTPv1 Sync message field offsets
const OFFSET_VERSION_PTP: usize = 0;
const OFFSET_VERSION_NETWORK: usize = 1;
const OFFSET_SUBDOMAIN: usize = 20;
const OFFSET_MESSAGE_TYPE: usize = 28;
const OFFSET_ORIGIN_SECONDS: usize = 40;
const OFFSET_ORIGIN_NANOSECONDS: usize = 44;
const OFFSET_GRANDMASTER_ID: usize = 62;
const OFFSET_SEQUENCE_ID: usize = 72;

/// Start the PTP listener. Returns a watch receiver for the latest offset.
/// Returns None if the socket can't be bound (port in use, no permission).
pub async fn start_ptp_listener(
    interface_ip: Ipv4Addr,
) -> Option<watch::Receiver<Option<PtpOffset>>> {
    let bind_addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), PTP_PORT);
    
    let socket = match UdpSocket::bind(bind_addr).await {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to bind PTP socket to {}:{}: {}", bind_addr.ip(), bind_addr.port(), e);
            return None;
        }
    };

    if let Err(e) = socket.join_multicast_v4(PTP_MULTICAST, interface_ip) {
        warn!("Failed to join PTP multicast group: {}", e);
        return None;
    }

    info!("PTP listener started on {}:{}, joined multicast {}", 
          bind_addr.ip(), bind_addr.port(), PTP_MULTICAST);

    let (tx, rx) = watch::channel(None);

    tokio::spawn(async move {
        run_ptp_listener(socket, tx).await;
    });

    Some(rx)
}

async fn run_ptp_listener(
    socket: UdpSocket,
    tx: watch::Sender<Option<PtpOffset>>,
) {
    let mut buffer = vec![0u8; PTP_MESSAGE_SIZE];
    let mut last_grandmaster_id: Option<[u8; 6]> = None;

    loop {
        match socket.recv(&mut buffer).await {
            Ok(n) => {
                if n < PTP_MESSAGE_SIZE {
                    debug!("Received incomplete PTP message: {} bytes", n);
                    continue;
                }

                if let Some(offset) = parse_ptp_sync(&buffer, last_grandmaster_id) {
                    // Log on first sync or when grandmaster changes
                    if last_grandmaster_id != Some(offset.grandmaster_id) {
                        info!(
                            "PTP grandmaster changed: ID={:02x?}",
                            offset.grandmaster_id
                        );
                        last_grandmaster_id = Some(offset.grandmaster_id);
                    }

                    debug!(
                        "PTP Sync: GM={:02x?}, seq={}, offset_ns={}",
                        offset.grandmaster_id, offset.sequence_id, offset.offset_ns
                    );

                    let _ = tx.send(Some(offset));
                }
            }
            Err(e) => {
                warn!("Error receiving PTP message: {}", e);
            }
        }
    }
}

fn parse_ptp_sync(buffer: &[u8], _last_gm: Option<[u8; 6]>) -> Option<PtpOffset> {
    if buffer.len() < PTP_MESSAGE_SIZE {
        return None;
    }

    // Check version PTP
    let version_ptp = buffer[OFFSET_VERSION_PTP];
    if version_ptp != 1 {
        debug!("Invalid PTPv1 version: {}", version_ptp);
        return None;
    }

    // Check version Network
    let version_network = buffer[OFFSET_VERSION_NETWORK];
    if version_network != 1 {
        debug!("Invalid Network version (expected IPv4=1): {}", version_network);
        return None;
    }

    // Check message type (0x01 = Sync)
    let message_type = buffer[OFFSET_MESSAGE_TYPE];
    if message_type != 0x01 {
        debug!("Not a Sync message, type: {:#x}", message_type);
        return None;
    }

    // Extract origin timestamp (seconds + nanoseconds)
    let origin_seconds = u32::from_be_bytes([
        buffer[OFFSET_ORIGIN_SECONDS],
        buffer[OFFSET_ORIGIN_SECONDS + 1],
        buffer[OFFSET_ORIGIN_SECONDS + 2],
        buffer[OFFSET_ORIGIN_SECONDS + 3],
    ]) as u64;

    let origin_nanoseconds = u32::from_be_bytes([
        buffer[OFFSET_ORIGIN_NANOSECONDS],
        buffer[OFFSET_ORIGIN_NANOSECONDS + 1],
        buffer[OFFSET_ORIGIN_NANOSECONDS + 2],
        buffer[OFFSET_ORIGIN_NANOSECONDS + 3],
    ]) as u64;

    let origin_timestamp_ns = origin_seconds * 1_000_000_000 + origin_nanoseconds;

    // Measure local receive time in nanoseconds
    let local_recv_ns = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as u64,
        Err(_) => {
            warn!("SystemTime error: cannot determine current time");
            return None;
        }
    };

    let offset_ns = (local_recv_ns as i64) - (origin_timestamp_ns as i64);

    // Extract grandmaster clock identity (6 bytes)
    let mut grandmaster_id = [0u8; 6];
    grandmaster_id.copy_from_slice(&buffer[OFFSET_GRANDMASTER_ID..OFFSET_GRANDMASTER_ID + 6]);

    // Extract sequence ID (big-endian u16)
    let sequence_id = u16::from_be_bytes([
        buffer[OFFSET_SEQUENCE_ID],
        buffer[OFFSET_SEQUENCE_ID + 1],
    ]);

    Some(PtpOffset {
        grandmaster_id,
        sequence_id,
        offset_ns,
        recv_time: std::time::Instant::now(),
    })
}
