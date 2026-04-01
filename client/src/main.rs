mod api;
mod args;
mod player;
mod state;
mod ui;

use args::Args;
use clap::Parser;
use state::ClientState;
use ui::App;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let args = Args::parse();

    let state = ClientState::new(args.config.clone());
    let api = api::ApiClient::new(args.server.clone());
    let player = match player::AudioPlayer::new() {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("Audio player unavailable: {}", e);
            None
        }
    };

    let app = App::new(state, api, player);
    ui::run(app).await
}
