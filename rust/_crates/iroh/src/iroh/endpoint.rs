use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use anyhow::Result;
use iroh::discovery::pkarr::dht::DhtDiscovery;
use iroh::{Endpoint, SecretKey};

// spin up an ephemeral endpoint using the mainline
//  dht as a discovery mechanism
pub async fn create_ephemeral_endpoint() -> Endpoint {
    // Connect to the mainline DHT as an ephemeral node
    let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0); // Let system choose port
    let mainline_discovery = DhtDiscovery::builder()
        .build()
        .expect("failed to build mainline discovery");

    // Create the endpoint with our key and discovery

    Endpoint::builder()
        .discovery(Box::new(mainline_discovery))
        .bind_addr_v4(addr)
        .bind()
        .await
        .expect("failed to bind ephemeral endpoint")
}

pub async fn create_endpoint(socket_addr: SocketAddr, secret_key: SecretKey) -> Endpoint {
    // Convert the SocketAddr to a SocketAddrV4
    let addr = SocketAddrV4::new(
        socket_addr
            .ip()
            .to_string()
            .parse::<Ipv4Addr>()
            .unwrap_or(Ipv4Addr::UNSPECIFIED),
        socket_addr.port(),
    );

    let mainline_discovery = DhtDiscovery::builder()
        .secret_key(secret_key.clone())
        .build()
        .expect("failed to build mainline discovery");

    // Create the endpoint with our key and discovery

    Endpoint::builder()
        .secret_key(secret_key)
        .discovery(Box::new(mainline_discovery))
        .bind_addr_v4(addr)
        .bind()
        .await
        .expect("failed to bind ephemeral endpoint")
}

// Helper to wait for DERP relay assignment (optional)
pub async fn await_relay_region(endpoint: Endpoint) -> Result<()> {
    let t0 = std::time::Instant::now();
    loop {
        let addr = endpoint.node_addr().await?;

        if addr.relay_url().is_some() {
            break;
        }
        if t0.elapsed() > std::time::Duration::from_secs(10) {
            panic!("failed to setup iroh endpoint against relay")
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Ok(())
}
