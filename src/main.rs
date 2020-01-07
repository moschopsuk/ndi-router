use tokio::net::{TcpListener, TcpStream};
use std::{env, error::Error};
use tokio::stream::StreamExt;

use std::collections::HashMap;
use std::sync::{Mutex};

mod ndi;

struct VideoRouterState {
    map: Mutex<HashMap<String, String>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if !ndi::initialize() {
        panic!("Cannot initialize NDI libs");
    }

    // Parse the arguments, bind the TCP socket we'll be listening to, spin up
    // our worker threads, and start shipping sockets to those worker threads.
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9909".to_string());
    let mut server = TcpListener::bind(&addr).await?;
    let mut incoming = server.incoming();
    println!("Listening on: {}", addr);

    while let Some(Ok(stream)) = incoming.next().await {
        tokio::spawn(async move {
            if let Err(e) = process(stream).await {
                println!("failed to process connection; error = {}", e);
            }
        });
    }

    Ok(())
}

async fn process(stream: TcpStream) -> Result<(), Box<dyn Error>> {
    Ok(())
}