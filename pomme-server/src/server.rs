use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::config::{Rendering, ServerConfig};
use crate::plugin::{PluginEvent, PluginManager};

pub async fn run(config: ServerConfig) -> io::Result<()> {
    let listener = TcpListener::bind(config.address()).await?;
    let address = listener.local_addr()?;
    let plugins = Arc::new(PluginManager::load(&config.plugins_dir).map_err(io::Error::other)?);
    let players = Arc::new(AtomicUsize::new(0));
    let cancel = CancellationToken::new();
    let mut clients = JoinSet::new();

    tracing::info!(%address, plugins = plugins.len(), "server listening");
    plugins
        .emit(&PluginEvent::ServerStarted {
            address: address.to_string(),
        })
        .await;

    if config.rendering == Rendering::On {
        tokio::spawn(crate::renderer::run(
            Arc::clone(&players),
            cancel.child_token(),
        ));
    }

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, peer) = result?;
                let plugins = Arc::clone(&plugins);
                let players = Arc::clone(&players);
                clients.spawn(async move {
                    players.fetch_add(1, Ordering::Relaxed);
                    let peer = peer.to_string();
                    plugins.emit(&PluginEvent::ClientConnected { peer: &peer }).await;
                    if let Err(error) = serve_client(stream).await {
                        tracing::debug!(%peer, %error, "client disconnected with an error");
                    }
                    plugins.emit(&PluginEvent::ClientDisconnected { peer: &peer }).await;
                    players.fetch_sub(1, Ordering::Relaxed);
                });
            }
            _ = tokio::signal::ctrl_c() => break,
            Some(result) = clients.join_next(), if !clients.is_empty() => {
                if let Err(error) = result {
                    tracing::warn!(%error, "client task failed");
                }
            }
        }
    }

    cancel.cancel();
    clients.abort_all();
    while clients.join_next().await.is_some() {}
    plugins.emit(&PluginEvent::ServerStopping).await;
    tracing::info!("server stopped");
    Ok(())
}

/// The initial transport is a line protocol suitable for health checks and
/// administration. Minecraft packet handling can be layered onto this accept
/// loop.
async fn serve_client(stream: TcpStream) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    write.write_all(b"POMME 1 READY\n").await?;
    while let Some(line) = lines.next_line().await? {
        match line.trim() {
            "PING" => write.write_all(b"PONG\n").await?,
            "QUIT" => {
                write.write_all(b"BYE\n").await?;
                break;
            }
            _ => write.write_all(b"ERROR unknown command\n").await?,
        }
    }
    Ok(())
}
