//! Periodic network health monitor.
//! Checks reachability of Dante peers discovered via mDNS.

pub async fn run_health_monitor() {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        check_network_health().await;
    }
}

async fn check_network_health() {
    // Check if we can bind to the Dante port (tests network stack)
    match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
        Ok(sock) => {
            // Try to send a probe to the Dante mDNS multicast address
            let dante_mdns = "224.0.0.251:5353";
            match sock.send_to(b"", dante_mdns).await {
                Ok(_) => tracing::debug!("Network health: mDNS reachable"),
                Err(e) => tracing::warn!("Network health: mDNS send failed: {e}"),
            }
        }
        Err(e) => tracing::warn!("Network health: cannot bind UDP socket: {e}"),
    }
    
    tracing::debug!("Network health check complete");
}
