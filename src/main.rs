use tracing::{debug, info};
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

#[tokio::main]
async fn main() {
    // Subscribe to traces.
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()) // Read trace levels from RUST_LOG env var.
        .init();

    // The only valid form for async hello world :D
    info!("Hello, World!");
    debug!("Путин е копеле");
}
