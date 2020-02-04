use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex};
use tokio::stream::{Stream, StreamExt};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};
use log4rs;
use futures::SinkExt;
use log::{error, info, debug};
use std::{env, error::Error, mem};
use std::sync::Arc;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

mod ndi;
mod videohub;
mod peer;
mod shared;

use crate::videohub::{VideoHub};
use crate::peer::{Peer};
use crate::shared::{Shared};
use crate::ndi::{FindInstance, RouteInstance};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const NUM_OUTPUTS: usize = 16;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();

    info!("starting ndi-router {}", VERSION);

    if !ndi::initialize() {
        panic!("Cannot initialize NDI libs");
    }

    let mut find = match FindInstance::builder().show_local_sources(true).build() {
        None => panic!(Some("Cannot initialize NDI finder")),
        Some(find) => find,
    };

    let new_sources = find.wait_for_sources(100);
    let sources = find.get_current_sources();
    let mut outputs  = vec![];
    let mut inputs = vec![];
    let mut video_hub = VideoHub::new(sources.len(), NUM_OUTPUTS);

    info!("Found {} NDI sources", sources.len());

    if new_sources {
        let mut i : usize = 0;
        for source in &sources {
            let label = source.ndi_name().to_owned();
            let ip = source.ip_address().to_owned();
            debug!("Found source '{}' {} ({})",  i, label, ip);
            video_hub.set_input_label(i, label);
            inputs.push(source.to_owned());
            i += 1;
        }
    } else {
        error!("No NDI sources found");
        return Ok(())
    }

    for x in 0..NUM_OUTPUTS {
        let name = format!("NDI output {}", x);
        let route = match RouteInstance::builder(name.as_str()).build() {
            None => panic!(Some("Cannot create NDI route")),
            Some(find) => find,
        };

        outputs.push(route);
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

#[derive(Debug)]
pub enum Message {
    /// A message that should be broadcasted to others.
    Received(Vec<String>),

    Broadcast(String),
}

// Peer implements `Stream` in a way that polls both the `Rx`, and `Framed` types.
// A message is produced whenever an event is ready until the `Framed` stream returns `None`.
impl Stream for Peer {
    type Item = Result<Message, LinesCodecError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {

        if let Poll::Ready(Some(v)) = Pin::new(&mut self.rx).poll_next(cx) {
            return Poll::Ready(Some(Ok(Message::Broadcast(v))));
        }

        // Secondly poll the `Framed` stream.
        let result: Option<_> = futures::ready!(Pin::new(&mut self.lines).poll_next(cx));

        Poll::Ready(match result {
            // We've received a message we should broadcast to others.
            Some(Ok(message)) => {
                if message == "" {
                    Some(Ok(Message::Received(mem::replace(&mut self.buf, vec![]))))
                } else {
                    self.buf.push(message);
                    Some(Ok(Message::Received(vec!["None".to_owned()])))
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
    lines.send(video_hub.inital_status_dump()).await?;

    // Register our peer with state which internally sets up some channels.
    let mut peer = Peer::new(state.clone(), lines).await?;

    // Process incoming messages until our stream is exhausted by a disconnect.
    while let Some(result) = peer.next().await {
        match result {
            // A message was received from the current user, we should
            // broadcast this message to the other users.
            Ok(Message::Received(msg)) => {
                let command = msg.first().unwrap().as_str();
                match command {
                    "PING:" => {
                        debug!("sending ACK to {}", peer.addr);
                        peer.lines.send("ACK\n".to_owned()).await?
                    },
                    "VIDEO OUTPUT ROUTING:" => {
                        let mut split = msg[1].split_whitespace();
                        let mut state = state.lock().await;
                        let route = state.outputs.get(split.next().unwrap().parse::<usize>().unwrap());
                        let source = state.inputs.get(split.next().unwrap().parse::<usize>().unwrap());
                        
                        route.unwrap().clear();
                        route.unwrap().change(source.unwrap());
                        let update = format!("{}\n{}\n\n", command, msg[1]);
                        state.broadcast(addr, &update).await;
                        peer.lines.send("ACK\n".to_owned()).await?
                    },
                    "VIDEO OUTPUT LOCKS:" => {
                        println!("{}", msg[1]);
                        peer.lines.send("ACK\n".to_owned()).await?
                    }
                    _ => (),
                }
            },
            Ok(Message::Broadcast(msg)) => {
                peer.lines.send(msg).await?;
            }
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