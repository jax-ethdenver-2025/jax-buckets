use service::{spawn_service, ServiceConfig};

#[tokio::main]
async fn main() {
    println!("🚀 Starting basic HTTP server...");

    let config = ServiceConfig::default();

    spawn_service(&config).await;
}
