//! # Fineract Modernized Rust Server Binary
//!
//! Entry point for `fineract-server`.
//! Supports runtime PORT configuration, in-memory demo fixtures, Dual-DB connection pools,
//! and native Linux execution.

use std::env;
use std::net::SocketAddr;
use std::time::Instant;

use fineract_server_rs::{build_fineract_router, FineractAppState, FineractServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(18080);

    let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_string());

    let mut config = FineractServerConfig::default();
    config.bind_address = bind_address.clone();
    config.port = port;

    let state = FineractAppState::new(config);
    let app = build_fineract_router(state);

    let addr: SocketAddr = format!("{bind_address}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let elapsed = start_time.elapsed();
    eprintln!(
        "[fineract-server] Started native Fineract HTTP Gateway on http://{} in {:.2?}",
        addr, elapsed
    );

    axum::serve(listener, app).await?;
    Ok(())
}
