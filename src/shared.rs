use std::collections::HashMap;
use std::net::SocketAddr;

use crate::videohub::{VideoHub};
use crate::peer::{Tx};
use crate::ndi::{Source, RouteInstance};

/// Data that is shared between all peers in the chat server.
///
/// This is the set of `Tx` handles for all connected clients. Whenever a
/// message is received from a client, it is broadcasted to all peers by
/// iterating over the `peers` entries and sending a copy of the message on each
/// `Tx`.
pub struct Shared {
    pub peers: HashMap<SocketAddr, Tx>,
    pub video_hub: VideoHub,
    pub inputs: Vec<Source<'static>>,
    pub outputs: Vec<RouteInstance>,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    pub fn new(video_hub: VideoHub, inputs: Vec<Source<'static>>, outputs: Vec<RouteInstance>) -> Self {
        Shared {
            peers: HashMap::new(),
            video_hub,
            outputs,
            inputs
        }
    }

    /// Send a `LineCodec` encoded message to every peer, except
    /// for the sender.
    pub async fn broadcast(&mut self, sender: SocketAddr, message: &str) {
        for peer in self.peers.iter_mut() {
            if *peer.0 != sender {
                let _ = peer.1.send(message.into());
            }
        }
    }
}
