use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, ready},
    time::Duration,
};

use futures::{FutureExt, SinkExt, StreamExt, future::poll_fn};
use tokio::{net::TcpStream, time::Instant};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Bytes, Message as WsMessage, Utf8Bytes},
};

type Result<T> = std::result::Result<T, String>;

pub struct Connection {
    timeout_duration: Duration,
    timeout: Pin<Box<tokio::time::Sleep>>,
    ping: Pin<Box<tokio::time::Interval>>,
    client: Pin<Box<WebSocketStream<MaybeTlsStream<TcpStream>>>>,

    #[cfg(feature = "websocket_logging")]
    logger: std::sync::mpsc::Sender<Direction>,
}

impl Connection {
    pub async fn new(path: &str, timeout: Duration) -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let (client, _) = tokio::time::timeout(timeout, tokio_tungstenite::connect_async_with_config(path, None, true))
            .await
            .map_err(|x| format!("{path}: {x}"))?
            .map_err(|x| format!("{path}: {x}"))?;

        Ok(Self {
            timeout_duration: timeout,
            timeout: Box::pin(tokio::time::sleep(timeout)),
            ping: Box::pin(tokio::time::interval(timeout / 2)),
            client: Box::pin(client),

            #[cfg(feature = "websocket_logging")]
            logger: message_writer(path),
        })
    }

    pub fn reset_timeout(&mut self) {
        tokio::time::Sleep::reset(self.timeout.as_mut(), Instant::now() + self.timeout_duration);
    }

    pub fn send_now(&mut self, cx: &mut Context<'_>, message: WsMessage) -> Result<()> {
        // log::info!("Trying to send: {message:?}");
        if !matches!(self.client.poll_ready_unpin(cx), Poll::Ready(Ok(()))) {
            return Err("Write buffer full".to_string());
        }

        #[cfg(feature = "websocket_logging")]
        self.logger.send(Direction::Send(message.clone())).unwrap();

        self.client.start_send_unpin(message).map_err(|x| x.to_string())?;
        let _ = self.client.poll_flush_unpin(cx).map_err(|x| x.to_string())?;

        Ok(())
    }

    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Result<Utf8Bytes>> {
        let _ = self.client.poll_flush_unpin(cx).map_err(|x| x.to_string())?;

        while let Poll::Ready(x) = self.client.poll_next_unpin(cx).map_err(|x| format!("While receiving: {x}"))? {
            match x {
                Some(message) => {
                    #[cfg(feature = "websocket_logging")]
                    self.logger.send(Direction::Received(message.clone())).unwrap();
                    // We have received some message, resetting timeout,
                    self.reset_timeout();
                    match message {
                        WsMessage::Text(bytes) => return Poll::Ready(Ok(bytes)),
                        WsMessage::Binary(_) => todo!(),
                        WsMessage::Ping(items) => self.send_now(cx, WsMessage::Pong(items))?,
                        WsMessage::Pong(_) => (),
                        WsMessage::Close(_) => return Poll::Ready(Err("Connection closed".to_string())),
                        WsMessage::Frame(_) => unreachable!(),
                    };
                }
                None => return Poll::Ready(Err("Connection closed".to_string())),
            }
        }

        if self.timeout.poll_unpin(cx).is_ready() {
            return Poll::Ready(Err("Connection timeout hit".to_string()));
        }

        while self.ping.poll_tick(cx).is_ready() {
            self.send_now(cx, WsMessage::Ping(Bytes::from_static("123".as_bytes())))?;
        }

        Poll::Pending
    }

    pub fn send(&mut self, message: WsMessage) -> Result<()> {
        #[cfg(feature = "websocket_logging")]
        self.logger.send(Direction::Send(message.clone())).unwrap();

        self.client.start_send_unpin(message).map_err(|x| x.to_string())?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Utf8Bytes> {
        poll_fn(|cx| self.poll(cx)).await
    }
}

pub enum Event {
    Connected,
    Disconnected(String),
    Data(Utf8Bytes),
}

pub struct Reconnection {
    path: String,
    timeout: Duration,
    state: ReconnectState,
}

enum ReconnectState {
    Connecting(Pin<Box<dyn Future<Output = Result<Connection>>>>),
    Connected(Connection),
}

impl Reconnection {
    pub fn new(path: String, timeout: Duration) -> Reconnection {
        Self {
            path: path.clone(),
            timeout,
            state: ReconnectState::Connecting(Box::pin(async move { Connection::new(&path, timeout).await })),
        }
    }

    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Event> {
        match &mut self.state {
            ReconnectState::Connecting(pin) => match ready!(pin.poll_unpin(cx)) {
                Ok(x) => {
                    self.state = ReconnectState::Connected(x);
                    Poll::Ready(Event::Connected)
                }
                Err(err) => {
                    self.state = ReconnectState::Connecting(Box::pin(reconnect(self.path.clone(), self.timeout)));
                    Poll::Ready(Event::Disconnected(err.to_string()))
                }
            },
            ReconnectState::Connected(connection) => match ready!(connection.poll(cx)) {
                Ok(x) => Poll::Ready(Event::Data(x)),
                Err(err) => {
                    self.state = ReconnectState::Connecting(Box::pin(reconnect(self.path.clone(), self.timeout)));
                    Poll::Ready(Event::Disconnected(err.to_string()))
                }
            },
        }
    }

    pub fn send(&mut self, message: WsMessage) -> Result<()> {
        match &mut self.state {
            ReconnectState::Connecting(_) => Err("Connection closed".to_string()),
            ReconnectState::Connected(connection) => connection.send(message),
        }
    }

    pub fn send_now(&mut self, cx: &mut Context<'_>, message: WsMessage) -> Result<()> {
        match &mut self.state {
            ReconnectState::Connecting(_) => Err("Connection closed".to_string()),
            ReconnectState::Connected(connection) => connection.send_now(cx, message),
        }
    }
}

async fn reconnect(path: String, timeout: Duration) -> Result<Connection> {
    tokio::time::sleep(Duration::from_secs(5)).await;
    Connection::new(&path, timeout).await
}

#[cfg(feature = "websocket_logging")]
enum Direction {
    Send(WsMessage),
    Received(WsMessage),
}

#[cfg(feature = "websocket_logging")]
fn message_writer(path: &str) -> std::sync::mpsc::Sender<Direction> {
    use std::{
        fs::OpenOptions,
        hash::{DefaultHasher, Hash, Hasher},
        io::Write,
    };

    use chrono::Utc;

    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let path = hasher.finish().to_string();
    std::fs::create_dir_all("log").unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<Direction>();
    std::thread::spawn(move || {
        let mut f = OpenOptions::new().create(true).append(true).open(format!("log/{path}")).unwrap();
        while let Ok(data) = rx.recv() {
            let now = Utc::now();
            let formatted = now.format("%Y-%m-%d %H:%M:%S%.3f");
            f.write_all(formatted.to_string().as_bytes()).unwrap();

            match data {
                Direction::Send(message) => {
                    f.write_all("  SEND -> ".as_bytes()).unwrap();
                    match &message {
                        WsMessage::Text(_) => f.write_all("TEXT".as_bytes()).unwrap(),
                        WsMessage::Binary(_) => f.write_all("BINARY".as_bytes()).unwrap(),
                        WsMessage::Ping(_) => f.write_all("PING".as_bytes()).unwrap(),
                        WsMessage::Pong(_) => f.write_all("PONG".as_bytes()).unwrap(),
                        WsMessage::Close(_) => f.write_all("CLOSE".as_bytes()).unwrap(),
                        WsMessage::Frame(_) => f.write_all("FRAME".as_bytes()).unwrap(),
                    }
                    f.write_all(&message.into_data()).unwrap();
                    f.write_all("\n".as_bytes()).unwrap();
                }
                Direction::Received(message) => {
                    f.write_all("  RECEIVE <- ".as_bytes()).unwrap();
                    match &message {
                        WsMessage::Text(_) => f.write_all("TEXT".as_bytes()).unwrap(),
                        WsMessage::Binary(_) => f.write_all("BINARY".as_bytes()).unwrap(),
                        WsMessage::Ping(_) => f.write_all("PING".as_bytes()).unwrap(),
                        WsMessage::Pong(_) => f.write_all("PONG".as_bytes()).unwrap(),
                        WsMessage::Close(_) => f.write_all("CLOSE".as_bytes()).unwrap(),
                        WsMessage::Frame(_) => f.write_all("FRAME".as_bytes()).unwrap(),
                    }
                    f.write_all(&message.into_data()).unwrap();
                    f.write_all("\n".as_bytes()).unwrap();
                }
            }
            f.flush().unwrap();
        }
    });
    tx
}
