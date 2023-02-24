pub mod server;

use iced_futures::futures;
use iced_native::subscription::{self, Subscription};

use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use chrono::Local;

use async_tungstenite::tungstenite;
use std::fmt;

pub fn connect() -> Subscription<Event> {
    struct Connect;

    subscription::unfold(
        std::any::TypeId::of::<Connect>(),
        State::Disconnected("".to_string()),
        |state| async move {
            match state {
                State::Disconnected(url) => {
                    println!("State::Disconnected({})", url);
                    match async_tungstenite::tokio::connect_async(url.clone()).await {
                        Ok((websocket, _)) => {
                            let (sender, receiver) = mpsc::channel(100);
                            let sender_clone = sender.clone();
                            (
                                Some(Event::Connected(Connection(sender_clone))),
                                State::Connected(url, websocket, receiver),
                            )
                        }
                        Err(_) => {
                            let (sender, receiver) = mpsc::channel(100);
                            (
                                Some(Event::Reconnect(Connection(sender.clone()))),
                                State::Reconnect(sender.clone(), receiver),
                            )
                        }
                    }
                }
                State::Reconnect(sender, mut receiver) => loop {
                    let message = receiver.select_next_some().await;
                    match message {
                        Message::Reconnect(url) => {
                            println!("Reconnect got url:{url}");
                            return (
                                Some(Event::Reconnect(Connection(sender))),
                                State::Disconnected(url),
                            );
                        }
                        _ => continue,
                    }
                },
                State::Connected(url, mut websocket, mut input) => {
                    let mut fused_websocket = websocket.by_ref().fuse();

                    futures::select! {
                        received = fused_websocket.select_next_some() => {
                            match received {
                                Ok(tungstenite::Message::Text(message)) => {
                                    (
                                        Some(Event::MessageReceived(Message::User(message))),
                                        State::Connected(url, websocket, input)
                                    )
                                }
                                Ok(_) => {
                                    (None, State::Connected(url, websocket, input))
                                }
                                Err(_) => {
                                    (Some(Event::Disconnected), State::Disconnected(url))
                                }
                            }
                        }

                        message = input.select_next_some() => {
                            match message {
                                Message::Stop(_)=> {
                                    fused_websocket.close().await.unwrap();
                                    let (sender, receiver) = mpsc::channel(100);
                                    (
                                        Some(Event::Reconnect(Connection(sender.clone()))),
                                        State::Reconnect(sender.clone(), receiver),
                                    )
                                }
                                Message::User(info)=> {
                                    let result = websocket.send(tungstenite::Message::Text(info)).await;
                                    if result.is_ok() {
                                        (None, State::Connected(url, websocket, input))
                                    } else {
                                        (Some(Event::Disconnected), State::Disconnected(url))
                                    }
                                }
                                _ => (None, State::Disconnected(url))
                            }
                        }
                    }
                }
            }
        },
    )
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum State {
    Disconnected(String),
    Connected(
        String,
        async_tungstenite::WebSocketStream<async_tungstenite::tokio::ConnectStream>,
        mpsc::Receiver<Message>,
    ),
    Reconnect(mpsc::Sender<Message>, mpsc::Receiver<Message>),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(Connection),
    Disconnected,
    Reconnect(Connection),
    MessageReceived(Message),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<Message>);

impl Connection {
    pub fn send(&mut self, message: Message) {
        self.0
            .try_send(message)
            .expect("Send message to echo server");
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Connected(String),
    Disconnected,
    Reconnect(String),
    Stop(String),
    User(String),
}

impl Message {
    pub fn new(message: &str) -> Option<Self> {
        if message.is_empty() {
            None
        } else {
            Some(Self::User(message.to_string()))
        }
    }

    pub async fn get_url(url: String) -> Self {
        Message::Reconnect(url)
    }

    pub fn connected(url: String) -> Self {
        Message::Connected(url)
    }

    pub fn disconnected() -> Self {
        Message::Disconnected
    }

    // pub fn reconnect(url: String) -> Self {
    //     Message::Reconnect(url)
    // }

    pub fn stop(url: String) -> Self {
        Message::Stop(url)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let now = Local::now().format("%Y/%m/%d %H:%M:%S");
        match self {
            Message::Connected(url) => write!(f, "{now}\nConnected with {url} successfully!"),
            Message::Disconnected => {
                write!(f, "{now}\nConnection lost... Retrying...")
            }
            Message::Reconnect(url) => write!(f, "{now}\nReConnecte with {url}"),
            Message::Stop(url) => write!(f, "{now}\nStop with {url} successfully!"),
            Message::User(message) => write!(f, "{now}\n{message}"),
        }
    }
}
