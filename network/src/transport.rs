//! Transport layer.
//!

use std::time::Duration;

use crate::Error;

use libp2p::{
    PeerId, Transport,
    core::{
        muxing::StreamMuxerBox,
        transport::{Boxed, upgrade::Version},
    },
    dns,
    identity::Keypair,
    metrics::{BandwidthTransport, Registry},
    noise, yamux,
};

#[cfg(feature = "test")]
use libp2p::core::transport::memory;

#[cfg(not(feature = "test"))]
use libp2p::tcp::{self, Config};

pub type KoreTransport = Boxed<(PeerId, StreamMuxerBox)>;

/// Builds the transport.
///
/// # Arguments
///
/// * `registry` - The Prometheus registry.
/// * `peer_id` - The peer ID.
/// * `keys` - The keypair.
///
/// # Returns
///
/// The transport and relay client.
///
/// # Errors
///
/// If the transport cannot be built.
///
pub fn build_transport(
    registry: &mut Registry,
    keys: &Keypair,
) -> Result<KoreTransport, Error> {
    // Allow TCP transport.
    // port_reuse(true) for use the same port to send / receive communication.
    #[cfg(not(feature = "test"))]
    let transport = tcp::tokio::Transport::new(Config::default());
    #[cfg(feature = "test")]
    let transport = memory::MemoryTransport::default();

    // Allow the DNS transport.
    let transport = dns::tokio::Transport::system(transport)
        .map_err(|e| Error::Transport(format!("DNS error {:?}", e)))?;

    // Build the noise authentication.
    let noise = noise::Config::new(keys).map_err(|e| {
        Error::Transport(format!("Noise authentication {:?}", e))
    })?;

    // Upgrade the transport with the noise authentication and yamux multiplexing.
    let transport = transport
        .upgrade(Version::V1Lazy)
        .authenticate(noise)
        .multiplex(yamux::Config::default())
        .timeout(Duration::from_secs(10))
        .boxed();

    // Wrap the transport with bandwidth metrics for Prometheus.
    let transport = BandwidthTransport::new(transport, registry)
        .map(|(peer_id, conn), _| (peer_id, StreamMuxerBox::new(conn)));

    Ok(transport.boxed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_transport() {
        let mut registry = Registry::default();
        let keypair = Keypair::generate_ed25519();
        let result = build_transport(&mut registry, &keypair);

        assert!(result.is_ok());
    }
}
