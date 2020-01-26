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
use ndi::{FindInstance, RouteInstance, Source};
use log::{error, info, debug};
use log4rs;

mod videohub;
use videohub::VideoHub;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const NUM_OUTPUTS: usize = 16;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();

    info!("starting ndi-router {}", VERSION);

    if !ndi::initialize() {
        panic!("Cannot initialize NDI libs");
    }

    let mut find = match FindInstance::builder().build() {
        None => panic!(Some("Cannot initialize NDI finder")),
        Some(find) => find,
    };

    let new_sources = find.wait_for_sources(100);
    let sources = find.get_current_sources();

    let mut outputs  = vec![];
    let mut inputs = vec![];
    for x in 0..NUM_OUTPUTS {
        let name = format!("NDI output {}", x);
        let route = match RouteInstance::builder(name.as_str()).build() {
            None => panic!(Some("Cannot create NDI route")),
            Some(find) => find,
        };

        outputs.push(route);
    }


    let mut video_hub = VideoHub::new(sources.len(), NUM_OUTPUTS);

    debug!("Found {} NDI sources", sources.len());

    if new_sources {
        let mut i : usize = 0;
        for source in &sources {
            let label = source.ndi_name().to_owned();
            debug!("Adding source '{}' {}",  i, label);
            video_hub.set_input_label(i, label);
            inputs.push(source.to_owned());
            i += 1;
        }
    } else {
        error!("No NDI sources found");
        return Ok(())
    }

    let state = Arc::new(Mutex::new(Shared::new(video_hub, inputs, outputs)));

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
    video_hub: videohub::VideoHub,
    inputs: Vec<Source<'static>>,
    outputs: Vec<RouteInstance>,
}

struct Peer {
    /// The TCP socket wrapped with the `Lines` codec, defined below.
    ///
    /// This handles sending and receiving data on the socket. When using
    /// `Lines`, we can work at the line level instead of having to manage the
    /// raw byte operations.
    lines: Framed<TcpStream, LinesCodec>,

    buf: Vec<String>,

    addr: SocketAddr,

    /// Receive half of the message channel.
    ///
    /// This is used to receive messages from peers. When a message is received
    /// off of this `Rx`, it will be written to the socket.
    rx: Rx,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new(video_hub: VideoHub, inputs: Vec<Source<'static>>, outputs: Vec<RouteInstance>) -> Self {
        Shared {
            peers: HashMap::new(),
            video_hub,
            outputs,
            inputs
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

        Ok(Peer { lines, buf, rx, addr })
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


    let video_hub = state.lock().await.video_hub.clone();
    let mut initial_dump: Vec<String> = Vec::new();

    initial_dump.push(video_hub.clone().preamble());
    initial_dump.push(video_hub.clone().device_info());
    initial_dump.push(video_hub.clone().list_inputs());
    initial_dump.push(video_hub.clone().list_outputs());
    initial_dump.push(video_hub.clone().list_routes());
    initial_dump.push(video_hub.clone().list_locks());

    lines.send(initial_dump.join("")).await?;

    // Register our peer with state which internally sets up some channels.
    let mut peer = Peer::new(state.clone(), lines).await?;

    // Process incoming messages until our stream is exhausted by a disconnect.
    while let Some(result) = peer.next().await {
        match result {
            // A message was received from the current user, we should
            // broadcast this message to the other users.
            Ok(Message::Broadcast(msg)) => {
                match msg[0].as_str() {
                    "PING:" => {
                        debug!("sending ACK to {}", peer.addr);
                        peer.lines.send("ACK\n".to_owned()).await?
                    },
                    "VIDEO OUTPUT ROUTING:" => {
                        let mut split = msg[1].split_whitespace();
                        let state = state.lock().await;
                        let route = state.outputs.get(split.next().unwrap().parse::<usize>().unwrap());
                        let source = state.inputs.get(split.next().unwrap().parse::<usize>().unwrap());
                        
                        route.unwrap().change(source.unwrap());
                        peer.lines.send("ACK\n".to_owned()).await?
                    },
                    "VIDEO OUTPUT LOCKS:" => {
                        println!("{}", msg[1]);
                        peer.lines.send("ACK\n".to_owned()).await?
                    }
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
        info!("Client {} Disconnected", addr);
        let mut state = state.lock().await;
        state.peers.remove(&addr);
    }

    Ok(())
}