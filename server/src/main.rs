mod analyze;
mod api;
mod args;
mod scanner;
mod state;

use args::Args;
use clap::Parser;
use state::AppState;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let args = Args::parse();

    let state = AppState::new(args.config);

    let port = args.port;
    let strudel_port = args.strudel_port;

    {
        let mut config = state.config.write().await;
        config.port = port;
        config.strudel_port = strudel_port;
    }
    state.save().await;

    let router = api::create_router(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting ssmgr-server on {}", addr);
    info!("Strudel samples port: {}", strudel_port);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, router)
        .await
        .expect("Server failed");
}
