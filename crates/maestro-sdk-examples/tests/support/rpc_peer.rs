//! A prepared native child serving finite application RPC scenarios.

use crate::child::Scratch;
use maestro_app::{RpcClient, RpcClientOptions};
pub use maestro_test_process::rpc_peer::Scenario;
use serde_json::Value;
use std::{net::TcpListener, net::TcpStream, path::Path, rc::Rc};
use tokio::sync::mpsc;

/// The installed executable, release socket and their disposable directory.
pub struct Peer {
    /// The directory, removed with the peer.
    pub scratch: Scratch,
    /// The executable's path.
    entry: String,
    /// Out-of-band channel on which the peer reports what it observed.
    control: TcpListener,
}

impl Peer {
    /// Install the prepared executable in a fresh directory, without discovering build outputs.
    pub fn new() -> Self {
        let scratch = Scratch::new();
        let entry = scratch.join("peer");
        maestro_test_process::rpc_peer::install(Path::new(&entry), Scenario::Serve).unwrap();
        Self {
            scratch,
            entry,
            control: TcpListener::bind("127.0.0.1:0").unwrap(),
        }
    }

    /// Options that launch one compiled scenario on the actual client transport.
    pub fn options(&self, scenario: Scenario) -> RpcClientOptions {
        std::fs::write(
            format!("{}.fixture", self.entry),
            format!("rpc-peer\n{}\n", scenario.name()),
        )
        .unwrap();
        let mut options = RpcClientOptions {
            cli_path: Some(self.entry.clone()),
            ..RpcClientOptions::default()
        };
        options.env.insert(
            "PEER_PORT".into(),
            self.control.local_addr().unwrap().port().to_string(),
        );
        options
    }

    /// Accept a native peer's control connection, without a timed receive or polling.
    ///
    /// This blocks the calling thread; a peer that exits before connecting leaves the test
    /// waiting for the test runner's hang limit.
    pub fn control(&self) -> TcpStream {
        self.control.accept().unwrap().0
    }
}

/// Every event the client delivers from now on, in order.
pub fn events(client: &RpcClient) -> mpsc::UnboundedReceiver<Value> {
    let (sender, receiver) = mpsc::unbounded_channel();
    // The registration stays for the client's lifetime.
    let _remove = client.on_event(Rc::new(move |event: &Value| {
        let _ = sender.send(event.clone());
        Ok(())
    }));
    receiver
}

/// The next delivered event whose `type` is `kind`.
pub async fn next_of(events: &mut mpsc::UnboundedReceiver<Value>, kind: &str) -> Value {
    loop {
        let event = events.recv().await.unwrap();
        if event["type"] == kind {
            return event;
        }
    }
}
