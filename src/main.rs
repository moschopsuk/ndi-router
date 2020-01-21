use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::stream::{Stream, StreamExt};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

use std::{env, error::Error};
use std::collections::HashMap;
use std::sync::Arc;
use std::net::SocketAddr;
use std::pin::Pin;
use std::io;
use std::task::{Context, Poll};
use std::mem;

mod ndi;

use futures::SinkExt;
use ndi::{FindInstance, Source};
use log::{error, info, warn};
use log4rs;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();

    info!("starting ndi-router {}", VERSION);

    if !ndi::initialize() {
        panic!("Cannot initialize NDI libs");
    }

    let mut find = match FindInstance::builder().build() {
        None => panic!(Some("Cannot initialize NDI libs")),
        Some(find) => find,
    };

    let new_sources = find.wait_for_sources(100);
    let sources = find.get_current_sources();

    if new_sources {
        for source in &sources {
            info!("Found source '{}' with IP {}",  source.ndi_name(), source.ip_address());
        }
    } else {
        error!("No NDI sources found");
        return Ok(())
    }

    let state = Arc::new(Mutex::new(Shared::new()));

    // Parse the arguments, bind the TCP socket we'll be listening to, spin up
    // our worker threads, and start shipping sockets to those worker threads.
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9990".to_string());

    let mut listener = TcpListener::bind(&addr).await?;

    info!("server running on {}", addr);

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;

        // Clone a handle to the `Shared` state for the new connection.
        let state = Arc::clone(&state);

        // Spawn our handler to be run asynchronously.
        tokio::spawn(async move {
            if let Err(e) = process(state, stream, addr).await {
                println!("an error occured; error = {:?}", e);
            }
        });
    }
}

/// Shorthand for the transmit half of the message channel.
type Tx = mpsc::UnboundedSender<String>;

/// Shorthand for the receive half of the message channel.
type Rx = mpsc::UnboundedReceiver<String>;

/// Data that is shared between all peers in the chat server.
///
/// This is the set of `Tx` handles for all connected clients. Whenever a
/// message is received from a client, it is broadcasted to all peers by
/// iterating over the `peers` entries and sending a copy of the message on each
/// `Tx`.
struct Shared {
    peers: HashMap<SocketAddr, Tx>,
    ndi_sources: HashMap<u8, String>,
    routing: HashMap<u8, u8>,
}

struct Peer {
    /// The TCP socket wrapped with the `Lines` codec, defined below.
    ///
    /// This handles sending and receiving data on the socket. When using
    /// `Lines`, we can work at the line level instead of having to manage the
    /// raw byte operations.
    lines: Framed<TcpStream, LinesCodec>,

    buf: Vec<String>,

    /// Receive half of the message channel.
    ///
    /// This is used to receive messages from peers. When a message is received
    /// off of this `Rx`, it will be written to the socket.
    rx: Rx,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            peers: HashMap::new(),
            ndi_sources: HashMap::new(),
            routing: HashMap::new()
        }
    }

    /// Send a `LineCodec` encoded message to every peer, except
    /// for the sender.
    async fn broadcast(&mut self, sender: SocketAddr, message: &str) {
        for peer in self.peers.iter_mut() {
            if *peer.0 != sender {
                let _ = peer.1.send(message.into());
            }
        }
    }
}

/// The state for each connected client.
impl Peer {
    /// Create a new instance of `Peer`.
    async fn new(
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

        Ok(Peer { lines, buf, rx })
    }
}

#[derive(Debug)]
enum Message {
    /// A message that should be broadcasted to others.
    Broadcast(Vec<String>),
}

// Peer implements `Stream` in a way that polls both the `Rx`, and `Framed` types.
// A message is produced whenever an event is ready until the `Framed` stream returns `None`.
impl Stream for Peer {
    type Item = Result<Message, LinesCodecError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {

        // Secondly poll the `Framed` stream.
        let result: Option<_> = futures::ready!(Pin::new(&mut self.lines).poll_next(cx));

        Poll::Ready(match result {
            // We've received a message we should broadcast to others.
            Some(Ok(message)) => {
                if message == "" {
                    Some(Ok(Message::Broadcast(mem::replace(&mut self.buf, vec![]))))
                } else {
                    self.buf.push(message);
                    Some(Ok(Message::Broadcast(vec!["None".to_owned()])))
                }
            },

            // An error occured.
            Some(Err(e)) => Some(Err(e)),

            // The stream has been exhausted.
            None => None,
        })
    }
}

/// Process an individual chat client
async fn process(
    state: Arc<Mutex<Shared>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    info!("New videohub controller connected: {}", addr);
    
    let mut lines = Framed::new(stream, LinesCodec::new());

    let mut device_info: Vec<String> = Vec::new();
    device_info.push(String::from("PROTOCOL PREAMBLE:"));
    device_info.push(format!("Version: {}", 2.7));
    device_info.push(format!(""));
    device_info.push(format!("VIDEOHUB DEVICE:"));
    device_info.push(format!("Device present: true"));
    device_info.push(format!("Model name: Blackmagic Smart Videohub"));
    device_info.push(format!("Video inputs: 16"));
    device_info.push(format!("Video processing units: 0"));
    device_info.push(format!("Video outputs: 16"));
    device_info.push(format!("Video monitoring outputs: 0"));
    device_info.push(format!("Serial ports: 0"));
    device_info.push(format!(""));

    device_info.push(format!("INPUT LABELS:"));
    for x in 0..16 {
        device_info.push(format!("{} NDI Input {}", x, (x + 1)));
    }
    device_info.push(format!(""));

    device_info.push(format!("OUTPUT LABELS:"));
    for x in 0..16 {
        device_info.push(format!("{} NDI Output {}", x, (x + 1)));
    }
    device_info.push(format!(""));

    device_info.push(format!("VIDEO OUTPUT ROUTING:"));
    for x in 0..16 {
        device_info.push(format!("{} {}", 0, (x + 1)));
    }
    device_info.push(format!(""));

    device_info.push(format!("VIDEO OUTPUT LOCKS:"));
    for x in 0..16 {
        device_info.push(format!("{} U", x));
    }
    device_info.push(format!(""));

    lines.send(device_info.join("\n")).await?;


    // Register our peer with state which internally sets up some channels.
    let mut peer = Peer::new(state.clone(), lines).await?;

    // Process incoming messages until our stream is exhausted by a disconnect.
    while let Some(result) = peer.next().await {
        match result {
            // A message was received from the current user, we should
            // broadcast this message to the other users.
            Ok(Message::Broadcast(msg)) => {
                println!("Broadcast {:?}", msg);
                match msg[0].as_str() {
                    "PING:" => {
                        info!("sending ACK to client");
                        peer.lines.send("ACK\n".to_owned()).await?
                    },
                    "VIDEO OUTPUT ROUTING:" => println!("{}", msg[1]),
                    _ => (),
                }
            },
            Err(e) => {
                println!(
                    "an error occured while processing messages error = {:?}",
                    e
                );
            }
        }
    }

    // If this section is reached it means that the client was disconnected!
    // Let's let everyone still connected know about it.
    {
        println!("Client Disconnected");
        let mut state = state.lock().await;
        state.peers.remove(&addr);
    }

    Ok(())
}