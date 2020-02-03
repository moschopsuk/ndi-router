use tokio::net::{TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_util::codec::{Framed, LinesCodec};
use std::io;
use std::sync::Arc;

use std::net::SocketAddr;

use crate::shared::{Shared};

/// Shorthand for the transmit half of the message channel.
pub type Tx = mpsc::UnboundedSender<String>;

/// Shorthand for the receive half of the message channel.
pub type Rx = mpsc::UnboundedReceiver<String>;

pub struct Peer {
    /// The TCP socket wrapped with the `Lines` codec, defined below.
    ///
    /// This handles sending and receiving data on the socket. When using
    /// `Lines`, we can work at the line level instead of having to manage the
    /// raw byte operations.
    pub lines: Framed<TcpStream, LinesCodec>,

    pub buf: Vec<String>,

    pub addr: SocketAddr,

    /// Receive half of the message channel.
    ///
    /// This is used to receive messages from peers. When a message is received
    /// off of this `Rx`, it will be written to the socket.
    pub rx: Rx,
}

/// The state for each connected client.
impl Peer {
    /// Create a new instance of `Peer`.
    pub async fn new(
        state: Arc<Mutex<Shared>>,
        lines: Framed<TcpStream, LinesCodec>,
    ) -> io::Result<Peer> {
        // Get the client socket address
        let addr = lines.get_ref().peer_addr()?;

        // Create a channel for this peer
        let (tx, rx) = mpsc::unbounded_channel();

        let buf = Vec::new();

        // Add an entry for this `Peer` in the shared state map.
        state.lock().await.peers.insert(addr, tx);

        Ok(Peer { lines, buf, rx, addr })
    }
}