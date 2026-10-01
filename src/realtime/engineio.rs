//! A minimal Engine.IO v4 / Socket.IO v5 client (websocket transport only), enough to talk
//! to Nightscout's socket.io 4.x server.
//!
//! Wire format, for reference:
//! - Engine.IO packets: `0` open (JSON handshake), `1` close, `2` ping, `3` pong, `4` message,
//!   `6` noop. With v4 the *server* pings and the client answers `3`.
//! - Socket.IO packets ride inside `4`: `0` connect, `1` disconnect, `2` event, `3` ack,
//!   `4` connect error. A non-default namespace is written `/name,` right after the type,
//!   followed by an optional ack id and a JSON payload, e.g. `42/storage,1["subscribe",{…}]`.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use url::Url;

use crate::error::{Error, Result};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

fn realtime_error(message: impl Into<String>) -> Error {
    Error::Transport(message.into().into())
}

/// What arrived on the namespace.
#[derive(Debug)]
pub(crate) enum Incoming {
    /// `42…["name", args…]`
    Event { name: String, args: Vec<Value> },
    /// `43…<id>[args…]`
    Ack { id: u64, args: Vec<Value> },
    /// The server disconnected the namespace (`41`), which is how Nightscout rejects root
    /// namespace credentials. Network failures are errors instead, so they stay retryable.
    Disconnected,
}

#[derive(Deserialize)]
struct Handshake {
    #[serde(rename = "pingInterval", default = "default_ping_interval")]
    ping_interval: u64,
    #[serde(rename = "pingTimeout", default = "default_ping_timeout")]
    ping_timeout: u64,
}

const fn default_ping_interval() -> u64 {
    25_000
}

const fn default_ping_timeout() -> u64 {
    20_000
}

/// One Socket.IO namespace over its own Engine.IO websocket.
pub(crate) struct Socket {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// `""` for the default namespace, otherwise `"/storage,"`.
    prefix: String,
    /// Longest silence tolerated before the connection is considered dead.
    liveness: Duration,
    next_ack: u64,
}

impl Socket {
    /// Opens the websocket, completes the Engine.IO handshake and joins `namespace`.
    pub(crate) async fn connect(base: &Url, namespace: &str, user_agent: &str) -> Result<Self> {
        let mut url = base.join("socket.io/")?;
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme)
            .map_err(|()| Error::InvalidUrl(format!("cannot use {scheme} with {base}")))?;
        url.query_pairs_mut()
            .append_pair("EIO", "4")
            .append_pair("transport", "websocket");

        let mut request = url
            .as_str()
            .into_client_request()
            .map_err(|e| realtime_error(format!("invalid socket.io URL: {e}")))?;
        if let Ok(ua) = HeaderValue::from_str(user_agent) {
            request.headers_mut().insert("user-agent", ua);
        }

        let (ws, _) =
            tokio::time::timeout(HANDSHAKE_TIMEOUT, tokio_tungstenite::connect_async(request))
                .await
                .map_err(|_| Error::Timeout)?
                .map_err(|e| realtime_error(format!("websocket connection failed: {e}")))?;

        let prefix = if namespace == "/" {
            String::new()
        } else {
            format!("{namespace},")
        };
        let mut socket = Self {
            ws,
            prefix,
            liveness: HANDSHAKE_TIMEOUT,
            next_ack: 0,
        };

        let open = socket.next_text().await?;
        let handshake: Handshake = open
            .strip_prefix('0')
            .and_then(|json| serde_json::from_str(json).ok())
            .ok_or_else(|| realtime_error(format!("unexpected Engine.IO handshake: {open}")))?;
        socket.liveness = Duration::from_millis(handshake.ping_interval + handshake.ping_timeout);

        socket.send(format!("40{}", socket.prefix)).await?;
        loop {
            let text = socket.next_text().await?;
            match text.as_bytes() {
                [b'2'] => socket.send("3".into()).await?,
                [b'4', b'0', ..] if socket.matches_namespace(&text[2..]) => break,
                [b'4', b'4', ..] if socket.matches_namespace(&text[2..]) => {
                    return Err(Error::Unauthorized {
                        message: format!("socket.io namespace refused: {}", &text[2..]),
                    });
                }
                _ => {}
            }
        }
        Ok(socket)
    }

    fn matches_namespace(&self, rest: &str) -> bool {
        if self.prefix.is_empty() {
            !rest.starts_with('/')
        } else {
            rest.starts_with(&self.prefix)
        }
    }

    async fn send(&mut self, text: String) -> Result<()> {
        self.ws
            .send(Message::text(text))
            .await
            .map_err(|e| realtime_error(format!("websocket send failed: {e}")))
    }

    /// Emits `event` with `args`; returns the ack id when `with_ack`.
    pub(crate) async fn emit(
        &mut self,
        event: &str,
        args: Vec<Value>,
        with_ack: bool,
    ) -> Result<Option<u64>> {
        let mut payload = Vec::with_capacity(args.len() + 1);
        payload.push(Value::String(event.to_owned()));
        payload.extend(args);
        let ack = with_ack.then(|| {
            self.next_ack += 1;
            self.next_ack
        });
        let id = ack.map(|id| id.to_string()).unwrap_or_default();
        let text = format!("42{}{id}{}", self.prefix, Value::Array(payload));
        self.send(text).await?;
        Ok(ack)
    }

    /// Next text frame, with a timeout.
    async fn next_text(&mut self) -> Result<String> {
        loop {
            let frame = tokio::time::timeout(self.liveness, self.ws.next())
                .await
                .map_err(|_| Error::Timeout)?;
            match frame {
                Some(Ok(Message::Text(text))) => return Ok(text.as_str().to_owned()),
                Some(Ok(Message::Ping(data))) => {
                    let _ = self.ws.send(Message::Pong(data)).await;
                }
                Some(Ok(Message::Close(_))) | None => {
                    return Err(realtime_error("the socket.io connection was closed"));
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(realtime_error(format!("websocket error: {e}"))),
            }
        }
    }

    /// Next event or ack on this namespace, answering heartbeats along the way.
    ///
    /// # Errors
    ///
    /// [`Error::Timeout`] when the server stops pinging, and [`Error::Transport`] when the
    /// connection fails or Engine.IO closes the session.
    pub(crate) async fn next(&mut self) -> Result<Incoming> {
        loop {
            let text = self.next_text().await?;
            let bytes = text.as_bytes();
            match bytes.first() {
                Some(b'2') => self.send("3".into()).await?,
                Some(b'1') => {
                    return Err(realtime_error("the server closed the Engine.IO session"));
                }
                Some(b'4') => {
                    if let Some(incoming) = self.parse_socketio(&text[1..]) {
                        return Ok(incoming);
                    }
                }
                _ => {}
            }
        }
    }

    fn parse_socketio(&self, packet: &str) -> Option<Incoming> {
        let kind = packet.as_bytes().first().copied()?;
        let mut rest = &packet[1..];
        if rest.starts_with('/') {
            let (ns, tail) = rest.split_once(',')?;
            if format!("{ns},") != self.prefix {
                return None;
            }
            rest = tail;
        } else if !self.prefix.is_empty() {
            return None;
        }
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let id = rest[..digits].parse::<u64>().ok();
        let payload = &rest[digits..];
        match kind {
            b'2' => {
                let mut items: Vec<Value> = serde_json::from_str(payload).ok()?;
                if items.is_empty() {
                    return None;
                }
                let name = items.remove(0).as_str()?.to_owned();
                Some(Incoming::Event { name, args: items })
            }
            b'3' => Some(Incoming::Ack {
                id: id?,
                args: serde_json::from_str(payload).unwrap_or_default(),
            }),
            b'1' => Some(Incoming::Disconnected),
            _ => None,
        }
    }

    /// Emits `event` and waits (up to `timeout`) for its ack. Events that arrive first
    /// (Nightscout sends the initial `dataUpdate` before acknowledging `authorize`) are
    /// returned alongside the ack.
    ///
    /// Only a namespace disconnect in reply means the credentials were rejected; a dropped
    /// connection is returned as a transport error so reconnect loops keep trying.
    pub(crate) async fn call(
        &mut self,
        event: &str,
        args: Vec<Value>,
        timeout: Duration,
    ) -> Result<(Vec<Value>, Vec<(String, Vec<Value>)>)> {
        let ack = self.emit(event, args, true).await?;
        let deadline = tokio::time::Instant::now() + timeout;
        let mut early = Vec::new();
        loop {
            let incoming = tokio::time::timeout_at(deadline, self.next())
                .await
                .map_err(|_| Error::Timeout)??;
            match incoming {
                Incoming::Ack { id, args } if Some(id) == ack => return Ok((args, early)),
                Incoming::Event { name, args } => early.push((name, args)),
                Incoming::Disconnected => {
                    return Err(Error::Unauthorized {
                        message: format!(
                            "Nightscout disconnected the socket after `{event}` (credentials rejected)"
                        ),
                    });
                }
                _ => {}
            }
        }
    }

    pub(crate) async fn close(mut self) {
        let _ = self.ws.close(None).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::net::TcpListener;

    /// A one-shot socket.io server: completes the handshake, reads one event, then sends
    /// `reply` (if any) and hangs up.
    async fn server(reply: Option<&'static str>) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("address");
        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.expect("accept");
            let mut ws = tokio_tungstenite::accept_async(tcp).await.expect("upgrade");
            let open = r#"0{"sid":"e","pingInterval":25000,"pingTimeout":20000}"#;
            ws.send(Message::text(open)).await.expect("open");
            let _connect = ws.next().await;
            ws.send(Message::text(r#"40{"sid":"s"}"#))
                .await
                .expect("connect");
            let _event = ws.next().await;
            if let Some(reply) = reply {
                ws.send(Message::text(reply)).await.expect("reply");
            }
        });
        Url::parse(&format!("http://{addr}/")).expect("url")
    }

    async fn authorize(
        reply: Option<&'static str>,
    ) -> Result<(Vec<Value>, Vec<(String, Vec<Value>)>)> {
        let mut socket = Socket::connect(&server(reply).await, "/", "test").await?;
        socket
            .call("authorize", vec![json!({})], Duration::from_secs(5))
            .await
    }

    #[tokio::test]
    async fn a_dropped_connection_is_retryable_not_a_rejection() {
        let res = authorize(None).await;
        assert!(matches!(res, Err(Error::Transport(_))), "{res:?}");
    }

    #[tokio::test]
    async fn a_namespace_disconnect_is_a_rejection() {
        let res = authorize(Some("41")).await;
        assert!(matches!(res, Err(Error::Unauthorized { .. })), "{res:?}");
    }
}
