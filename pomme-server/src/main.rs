use clap::Parser;
use pomme_server::config::ServerConfig;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pomme_server=info".into()),
        )
        .init();

    if let Err(error) = pomme_server::server::run(ServerConfig::parse()).await {
        tracing::error!(%error, "server failed");
        std::process::exit(1);
    }
}
