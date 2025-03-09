//! Simple server that prints received bytes
//!
//! ## Usage
//!
//!     cargo run --bin server

use anyhow::Result;
use iroh::{
    endpoint::Connecting,
    protocol::{ProtocolHandler, Router},
    Endpoint
};
use n0_future::boxed::BoxFuture;

/// Each protocol is identified by its ALPN string.
///
/// The ALPN, or application-layer protocol negotiation, is exchanged in the connection handshake,
/// and the connection is aborted unless both nodes pass the same bytestring.
const ALPN: &[u8] = b"iroh-example/print/0";

#[tokio::main]
async fn main() -> Result<()> {
    let router = accept_side().await?;
    let node_addr = router.endpoint().node_addr().await?;
    println!("Listening on {:?}", node_addr.node_id.to_string());

    tokio::signal::ctrl_c().await?;
    router.shutdown().await?;
    Ok(())
}

async fn accept_side() -> Result<Router> {
    let endpoint = Endpoint::builder().discovery_n0().bind().await?;
    let router = Router::builder(endpoint).accept(ALPN, PrintBytes).spawn().await?;

    Ok(router)
}

#[derive(Debug, Clone)]
struct PrintBytes;

impl ProtocolHandler for PrintBytes {
    /// The `accept` method is called for each incoming connection for our ALPN.
    ///
    /// The returned future runs on a newly spawned tokio task, so it can run as long as
    /// the connection lasts.
    fn accept(&self, connecting: Connecting) -> BoxFuture<Result<()>> {
        Box::pin(async move {
            let connection = connecting.await?;
            let node_id = connection.remote_node_id()?;
            println!("New connection from {node_id}");

            let (mut send, mut recv) = connection.accept_bi().await?;

            // Read all data from the stream
            let data = recv.read_to_end(usize::MAX).await?;
            println!("Total bytes received: {}", data.len());
            
            // Send small acknowledgment
            send.write_all(b"received").await?;
            send.finish()?;

            connection.closed().await;
            Ok(())
        })
    }
}