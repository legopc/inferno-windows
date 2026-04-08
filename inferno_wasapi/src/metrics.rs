//! Prometheus metrics HTTP endpoint on port 9090.
//! Exposes: audio peaks, packet stats, uptime, clock mode.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct Metrics {
    pub uptime_start: std::time::Instant,
    pub packets_received: AtomicU64,
    pub packets_lost: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_bytes: AtomicU64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            uptime_start: std::time::Instant::now(),
            packets_received: AtomicU64::new(0),
            packets_lost: AtomicU64::new(0),
            tx_bytes: AtomicU64::new(0),
            rx_bytes: AtomicU64::new(0),
        })
    }
    
    pub fn render_prometheus(&self) -> String {
        let uptime = self.uptime_start.elapsed().as_secs();
        let pkts_rx = self.packets_received.load(Ordering::Relaxed);
        let pkts_lost = self.packets_lost.load(Ordering::Relaxed);
        let tx_bytes = self.tx_bytes.load(Ordering::Relaxed);
        let rx_bytes = self.rx_bytes.load(Ordering::Relaxed);
        
        format!(
            "# HELP inferno_uptime_seconds Time since service start\n\
             # TYPE inferno_uptime_seconds gauge\n\
             inferno_uptime_seconds {uptime}\n\
             # HELP inferno_packets_received_total RTP packets received\n\
             # TYPE inferno_packets_received_total counter\n\
             inferno_packets_received_total {pkts_rx}\n\
             # HELP inferno_packets_lost_total RTP packets lost\n\
             # TYPE inferno_packets_lost_total counter\n\
             inferno_packets_lost_total {pkts_lost}\n\
             # HELP inferno_tx_bytes_total TX bytes sent\n\
             # TYPE inferno_tx_bytes_total counter\n\
             inferno_tx_bytes_total {tx_bytes}\n\
             # HELP inferno_rx_bytes_total RX bytes received\n\
             # TYPE inferno_rx_bytes_total counter\n\
             inferno_rx_bytes_total {rx_bytes}\n"
        )
    }
}

pub async fn serve_metrics(metrics: Arc<Metrics>) {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    
    let listener = match TcpListener::bind("127.0.0.1:9090").await {
        Ok(l) => { tracing::info!("Prometheus metrics at http://127.0.0.1:9090/metrics"); l }
        Err(e) => { tracing::warn!("Could not bind metrics port 9090: {e}"); return; }
    };
    
    loop {
        match listener.accept().await {
            Ok((mut stream, addr)) => {
                let body = metrics.render_prometheus();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(), body
                );
                if let Err(e) = stream.write_all(response.as_bytes()).await {
                    tracing::debug!(remote=%addr, "Metrics endpoint write error: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Metrics endpoint accept error: {}", e);
            }
        }
    }
}
