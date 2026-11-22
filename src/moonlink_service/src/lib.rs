mod error;
mod logging;
mod rpc_server;

pub use error::Error;
pub use error::Result;

use moonlink_backend::MoonlinkBackend;
use moonlink_metadata_store::SqliteMetadataStore;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

#[derive(Debug)]
pub struct ServiceConfig {
    /// Base location for moonlink storage (including cache files, iceberg tables, etc).
    /// The RPC server listens on `{base_path}/moonlink.sock`.
    pub base_path: String,
}

pub async fn start_with_config(config: ServiceConfig) -> Result<()> {
    logging::init_logging();

    // Initialize moonlink backend.
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let sqlite_metadata_accessor = SqliteMetadataStore::new_with_directory(&config.base_path)
        .await
        .unwrap();
    let backend = MoonlinkBackend::new(
        config.base_path.clone(),
        /*data_server_uri=*/ None,
        Box::new(sqlite_metadata_accessor),
    )
    .await?;
    let backend = Arc::new(backend);

    // Start RPC server on Unix socket
    let socket_path = std::path::PathBuf::from(&config.base_path).join("moonlink.sock");
    tokio::select! {
        result = rpc_server::start_unix_server(backend, socket_path) => result,
        _ = sigterm.recv() => {
            info!("Received SIGTERM, shutting down...");
            Ok(())
        }
    }
}

#[cfg(test)]
mod schema_wire_golden;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod test;
