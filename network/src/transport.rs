// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Transport layer.
//!

use crate::Error;

use libp2p::{
    core::{
        muxing::StreamMuxerBox,
        transport::{memory, upgrade::Version, Boxed},
    },
    dns,
    identity::Keypair,
    metrics::{BandwidthTransport, Registry},
    noise,
    tcp::{self, Config},
    yamux, PeerId, Transport,
};

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
    _port_reuse: bool,
) -> Result<KoreTransport, Error> {
    // Build the noise authentication.
    let noise = noise::Config::new(keys).map_err(|e| {
        Error::Transport(format!("Noise authentication {:?}", e))
    })?;

    // Allow TCP transport.
    // port_reuse(true) for use the same port to send / receive communication.
    #[cfg(not(feature = "test"))]
    let transport = tcp::tokio::Transport::new(Config::default());
    #[cfg(feature = "test")]
    let transport =  memory::MemoryTransport::default();

    // Upgrade the transport with the noise authentication and yamux multiplexing.
    let transport = transport
        .upgrade(Version::V1)
        .authenticate(noise)
        .multiplex(yamux::Config::default());

    // Allow the DNS transport.
    let transport = dns::tokio::Transport::system(transport)
        .map_err(|e| Error::Transport(format!("DNS error {:?}", e)))?;

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
        let result = build_transport(&mut registry, &keypair, false);

        assert!(result.is_ok());
    }
}
