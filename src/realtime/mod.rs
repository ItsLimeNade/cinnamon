//! Live updates over Nightscout's socket.io channels (feature `realtime`).
//!
//! | Stream | Channel | Carries | Credentials |
//! |---|---|---|---|
//! | [`Realtime::storage`] | `/storage` | API v3 creates, updates, deletes | access token |
//! | [`Realtime::alarms`] | `/alarm` | alarms, announcements, clears | token or API secret |
//! | [`Realtime::updates`] | root namespace | `dataUpdate` deltas, including data uploaded through API v1 | token or API secret |
//! | [`Realtime::glucose`] | root namespace | each new sensor glucose value | token or API secret |
//!
//! Most CGM uploaders (xDrip+, Loop, Trio, OpenAPS) still write through API v1, whose
//! writes are **not** broadcast on `/storage`; use [`Realtime::updates`] or
//! [`Realtime::glucose`] to follow them.
//!
//! Subscriptions reconnect by themselves (exponential backoff) and yield a `Reconnected`
//! event afterwards: anything sent while disconnected is lost, so catch up with
//! [`Client::sync`](crate::Client::sync) when you see it.
//!
//! ```no_run
//! # async fn run(ns: cinnamon::Client) -> cinnamon::Result<()> {
//! use futures_util::StreamExt;
//!
//! let mut glucose = ns.realtime().glucose().await?;
//! while let Some(reading) = glucose.next().await {
//!     let reading = reading?;
//!     println!("{:?} mg/dL at {:?}", reading.mgdl, reading.mills);
//! }
//! # Ok(()) }
//! ```

mod engineio;
mod events;

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures_core::Stream;
use futures_util::future::BoxFuture;
use futures_util::stream::{self, BoxStream, StreamExt};
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tokio::sync::mpsc;

pub use events::{Alarm, AlarmEvent, DataUpdate, StorageEvent, UpdateEvent};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::model::CollectionName;
use crate::model::notification::Level;
use crate::model::properties::PropertySgv;
use engineio::{Incoming, Socket};

const SUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(15);
const MIN_BACKOFF: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const BUFFER: usize = 256;

/// Live update subscriptions: `ns.realtime()`.
#[derive(Debug, Clone)]
pub struct Realtime {
    client: Client,
}

impl Client {
    /// Live update subscriptions over socket.io.
    #[must_use]
    pub fn realtime(&self) -> Realtime {
        Realtime {
            client: self.clone(),
        }
    }
}

impl Realtime {
    /// Creates, updates and deletes made through API v3 in `collections`.
    ///
    /// # Errors
    ///
    /// [`Error::Unsupported`] without an access token, [`Error::Unauthorized`] when the
    /// token cannot read any requested collection, and connection errors.
    pub async fn storage<I>(&self, collections: I) -> Result<Subscription<StorageEvent>>
    where
        I: IntoIterator<Item = CollectionName>,
    {
        if !self.client.inner.credentials.is_access_token() {
            return Err(Error::Unsupported {
                feature: "storage socket",
                reason: "the /storage channel only accepts access tokens".into(),
            });
        }
        let names = collections
            .into_iter()
            .map(|c| c.name().to_owned())
            .collect();
        start(self.client.clone(), StorageChannel { collections: names }).await
    }

    /// Alarms, urgent alarms, announcements and clears. Acknowledge with
    /// [`Subscription::ack`].
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] when the credentials are rejected, and connection errors.
    pub async fn alarms(&self) -> Result<Subscription<AlarmEvent>> {
        start(self.client.clone(), AlarmChannel).await
    }

    /// `dataUpdate` messages from the root namespace: the full recent data first, then
    /// deltas as anything (including API v1 uploads) changes.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] when the credentials cannot read, and connection errors.
    pub async fn updates(&self) -> Result<Subscription<UpdateEvent>> {
        start(self.client.clone(), LegacyChannel).await
    }

    /// Each new sensor glucose value, oldest first. The first item is the latest reading
    /// already stored; after that, one item per new reading.
    ///
    /// # Errors
    ///
    /// Same as [`Realtime::updates`].
    pub async fn glucose(&self) -> Result<BoxStream<'static, Result<PropertySgv>>> {
        let updates = self.updates().await?;
        let mut newest: Option<i64> = None;
        Ok(updates
            .map(move |event| -> Vec<Result<PropertySgv>> {
                match event {
                    Err(err) => vec![Err(err)],
                    Ok(UpdateEvent::Data(update)) => {
                        let mut sgvs: Vec<PropertySgv> = update
                            .sgvs
                            .into_iter()
                            .filter(|s| {
                                let t = s.mills.map(|m| m.as_millis());
                                t.is_some() && newest.is_none_or(|n| t > Some(n))
                            })
                            .collect();
                        sgvs.sort_by_key(|s| s.mills);
                        if newest.is_none() && sgvs.len() > 1 {
                            sgvs.drain(..sgvs.len() - 1);
                        }
                        if let Some(last) = sgvs.last().and_then(|s| s.mills) {
                            newest = Some(last.as_millis());
                        }
                        sgvs.into_iter().map(Ok).collect()
                    }
                    Ok(_) => Vec::new(),
                }
            })
            .flat_map(stream::iter)
            .boxed())
    }
}

/// A live subscription. Poll it as a [`Stream`]; dropping it closes the connection.
#[must_use = "a subscription does nothing unless polled"]
pub struct Subscription<T> {
    rx: mpsc::Receiver<Result<T>>,
    commands: mpsc::UnboundedSender<(String, Vec<Value>)>,
    task: tokio::task::JoinHandle<()>,
}

impl<T> std::fmt::Debug for Subscription<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription").finish_non_exhaustive()
    }
}

impl<T> Stream for Subscription<T> {
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Subscription<AlarmEvent> {
    /// Silences alarms of `level` (and lower) in `group` for `silence`, for every client of
    /// the site. Needs the `notifications:*:ack` permission.
    ///
    /// # Errors
    ///
    /// [`Error::Transport`] when the subscription has stopped.
    pub fn ack(&self, level: Level, group: &str, silence: Duration) -> Result<()> {
        let millis = u64::try_from(silence.as_millis()).unwrap_or(u64::MAX);
        self.commands
            .send((
                "ack".into(),
                vec![json!(level.as_i8()), json!(group), json!(millis)],
            ))
            .map_err(|_| Error::Transport("the alarm subscription has stopped".into()))
    }
}

/// One kind of socket.io subscription.
trait Channel: Send + Sync + 'static {
    type Event: Send + 'static;
    const NAMESPACE: &'static str;

    /// Authorizes/subscribes on a fresh socket; returns events that arrived meanwhile.
    fn subscribe<'a>(
        &'a self,
        client: &'a Client,
        socket: &'a mut Socket,
    ) -> BoxFuture<'a, Result<Vec<Self::Event>>>;

    fn decode(&self, name: &str, args: Vec<Value>) -> Option<Self::Event>;

    fn reconnected() -> Self::Event;
}

fn user_agent() -> String {
    format!("cinnamon/{}", env!("CARGO_PKG_VERSION"))
}

async fn connect<C: Channel>(client: &Client, channel: &C) -> Result<(Socket, Vec<C::Event>)> {
    let mut socket = Socket::connect(&client.inner.base, C::NAMESPACE, &user_agent()).await?;
    let initial = channel.subscribe(client, &mut socket).await?;
    Ok((socket, initial))
}

async fn start<C: Channel>(client: Client, channel: C) -> Result<Subscription<C::Event>> {
    // The first connection happens here so bad credentials surface to the caller.
    let (socket, initial) = connect(&client, &channel).await?;
    let (tx, rx) = mpsc::channel(BUFFER);
    for event in initial {
        let _ = tx.try_send(Ok(event));
    }
    let (commands, command_rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(run(client, channel, socket, tx, command_rx));
    Ok(Subscription { rx, commands, task })
}

async fn run<C: Channel>(
    client: Client,
    channel: C,
    mut socket: Socket,
    tx: mpsc::Sender<Result<C::Event>>,
    mut commands: mpsc::UnboundedReceiver<(String, Vec<Value>)>,
) {
    let mut commands_open = true;
    loop {
        // Pump the current connection until it drops.
        loop {
            tokio::select! {
                incoming = socket.next() => match incoming {
                    Ok(Incoming::Event { name, args }) => {
                        if let Some(event) = channel.decode(&name, args) {
                            if tx.send(Ok(event)).await.is_err() {
                                socket.close().await;
                                return;
                            }
                        }
                    }
                    Ok(Incoming::Ack { .. }) => {}
                    Ok(Incoming::Disconnected) | Err(_) => break,
                },
                command = commands.recv(), if commands_open => match command {
                    Some((name, args)) => {
                        if socket.emit(&name, args, false).await.is_err() {
                            break;
                        }
                    }
                    None => commands_open = false,
                },
                () = tx.closed() => {
                    socket.close().await;
                    return;
                }
            }
        }

        // Reconnect with exponential backoff and jitter.
        let mut backoff = MIN_BACKOFF;
        loop {
            if tx.is_closed() {
                return;
            }
            let jitter = backoff.mul_f64(fastrand::f64() * 0.5);
            tokio::time::sleep(backoff + jitter).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
            match connect(&client, &channel).await {
                Ok((fresh, initial)) => {
                    socket = fresh;
                    if tx.send(Ok(C::reconnected())).await.is_err() {
                        return;
                    }
                    for event in initial {
                        if tx.send(Ok(event)).await.is_err() {
                            return;
                        }
                    }
                    break;
                }
                // Revoked credentials will not come back: report and stop.
                Err(err @ Error::Unauthorized { .. }) => {
                    let _ = tx.send(Err(err)).await;
                    return;
                }
                Err(_) => {}
            }
        }
    }
}

fn ack_object(args: &[Value]) -> Value {
    args.first().cloned().unwrap_or(Value::Null)
}

struct StorageChannel {
    collections: Vec<String>,
}

impl Channel for StorageChannel {
    type Event = StorageEvent;
    const NAMESPACE: &'static str = "/storage";

    fn subscribe<'a>(
        &'a self,
        client: &'a Client,
        socket: &'a mut Socket,
    ) -> BoxFuture<'a, Result<Vec<StorageEvent>>> {
        Box::pin(async move {
            let token = client
                .inner
                .credentials
                .access_token_secret()
                .map(|t| t.expose_secret().to_owned())
                .unwrap_or_default();
            let mut payload = json!({ "accessToken": token });
            if !self.collections.is_empty() {
                payload["collections"] = json!(self.collections);
            }
            let (ack, early) = socket
                .call("subscribe", vec![payload], SUBSCRIBE_TIMEOUT)
                .await?;
            let ack = ack_object(&ack);
            if ack["success"] != true {
                return Err(Error::Unauthorized {
                    message: ack["message"]
                        .as_str()
                        .unwrap_or("storage subscription refused")
                        .to_owned(),
                });
            }
            Ok(early
                .into_iter()
                .filter_map(|(n, a)| self.decode(&n, a))
                .collect())
        })
    }

    fn decode(&self, name: &str, args: Vec<Value>) -> Option<StorageEvent> {
        StorageEvent::decode(name, args)
    }

    fn reconnected() -> StorageEvent {
        StorageEvent::Reconnected
    }
}

struct AlarmChannel;

impl Channel for AlarmChannel {
    type Event = AlarmEvent;
    const NAMESPACE: &'static str = "/alarm";

    fn subscribe<'a>(
        &'a self,
        client: &'a Client,
        socket: &'a mut Socket,
    ) -> BoxFuture<'a, Result<Vec<AlarmEvent>>> {
        Box::pin(async move {
            let creds = &client.inner.credentials;
            let payload = if let Some(token) = creds.access_token_secret() {
                json!({ "accessToken": token.expose_secret() })
            } else if let Some(hash) = creds.api_secret_hash() {
                json!({ "secret": hash.expose_secret() })
            } else {
                json!({})
            };
            let (ack, early) = socket
                .call("subscribe", vec![payload], SUBSCRIBE_TIMEOUT)
                .await?;
            let ack = ack_object(&ack);
            if ack["success"] != true {
                return Err(Error::Unauthorized {
                    message: ack["message"]
                        .as_str()
                        .unwrap_or("alarm subscription refused")
                        .to_owned(),
                });
            }
            Ok(early
                .into_iter()
                .filter_map(|(n, a)| self.decode(&n, a))
                .collect())
        })
    }

    fn decode(&self, name: &str, args: Vec<Value>) -> Option<AlarmEvent> {
        AlarmEvent::decode(name, args)
    }

    fn reconnected() -> AlarmEvent {
        AlarmEvent::Reconnected
    }
}

struct LegacyChannel;

impl Channel for LegacyChannel {
    type Event = UpdateEvent;
    const NAMESPACE: &'static str = "/";

    fn subscribe<'a>(
        &'a self,
        client: &'a Client,
        socket: &'a mut Socket,
    ) -> BoxFuture<'a, Result<Vec<UpdateEvent>>> {
        Box::pin(async move {
            let mut payload = json!({ "client": user_agent() });
            let creds = &client.inner.credentials;
            if creds.is_access_token() {
                let (jwt, _) = client.inner.bearer().await?;
                payload["token"] = json!(jwt.expose_secret());
            } else if let Some(hash) = creds.api_secret_hash() {
                payload["secret"] = json!(hash.expose_secret());
            }
            let (ack, early) = socket
                .call("authorize", vec![payload], SUBSCRIBE_TIMEOUT)
                .await?;
            if ack_object(&ack)["read"] != true {
                return Err(Error::Unauthorized {
                    message: "these credentials cannot read data over the socket".into(),
                });
            }
            Ok(early
                .into_iter()
                .filter_map(|(n, a)| self.decode(&n, a))
                .collect())
        })
    }

    fn decode(&self, name: &str, args: Vec<Value>) -> Option<UpdateEvent> {
        UpdateEvent::decode(name, args)
    }

    fn reconnected() -> UpdateEvent {
        UpdateEvent::Reconnected
    }
}
