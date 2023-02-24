mod echo;

use iced::alignment::{self, Alignment};
use iced::executor;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{Application, Color, Command, Element, Length, Settings, Subscription, Theme};
use once_cell::sync::Lazy;

pub fn main() -> iced::Result {
    WebSocket::run(Settings::default())
}

#[derive(Default)]
struct WebSocket {
    url: String,
    messages: Vec<echo::Message>,
    new_message: String,
    state: State,
}

#[derive(Debug, Clone)]
enum Message {
    Url(String),
    Connect,
    Reconnect(echo::Message),
    Stop,
    NewMessageChanged(String),
    Send(echo::Message),
    Echo(echo::Event),
    // Server,
}

impl Application for WebSocket {
    type Message = Message;
    type Theme = Theme;
    type Flags = ();
    type Executor = executor::Default;

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self::default(),
            // Command::perform(echo::server::run(), |_| Message::Server),
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("WebSocket Client")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Url(url) => {
                self.url = url;

                Command::none()
            }
            Message::Connect => {
                println!("self.url: {}", self.url);

                Command::perform(echo::Message::get_url(self.url.clone()), Message::Reconnect)
            }
            Message::Reconnect(message) => match &mut self.state {
                State::Connected(_) => Command::none(),
                State::Disconnected => Command::none(),
                State::Reconnect(connection) => {
                    connection.send(message);

                    Command::none()
                }
            },
            Message::Stop => match &mut self.state {
                State::Connected(connection) => {
                    self.messages.push(echo::Message::stop(self.url.clone()));

                    connection.send(echo::Message::Stop(self.url.clone()));

                    Command::none()
                }
                State::Disconnected => Command::none(),
                State::Reconnect(_) => Command::none(),
            },
            Message::NewMessageChanged(new_message) => {
                self.new_message = new_message;

                Command::none()
            }
            Message::Send(message) => match &mut self.state {
                State::Connected(connection) => {
                    self.new_message.clear();

                    if let echo::Message::User(info) = message.clone() {
                        self.messages
                            .push(echo::Message::User(format!("-> {}", info)));
                    }

                    connection.send(message);

                    Command::none()
                }
                State::Disconnected => Command::none(),
                State::Reconnect(_) => Command::none(),
            },
            Message::Echo(event) => match event {
                echo::Event::Connected(connection) => {
                    self.state = State::Connected(connection);

                    self.messages
                        .push(echo::Message::connected(self.url.clone()));

                    Command::none()
                }
                echo::Event::Disconnected => {
                    self.state = State::Disconnected;

                    self.messages.push(echo::Message::disconnected());

                    Command::none()
                }
                echo::Event::Reconnect(connection) => {
                    self.state = State::Reconnect(connection);

                    // self.messages
                    //     .push(echo::Message::reconnect(self.url.clone()));

                    Command::none()
                }
                echo::Event::MessageReceived(message) => {
                    if let echo::Message::User(info) = message {
                        self.messages
                            .push(echo::Message::User(format!("<- {}", info)));
                    }

                    scrollable::snap_to(MESSAGE_LOG.clone(), scrollable::RelativeOffset::END)
                }
            },
            // Message::Server => Command::none(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        echo::connect().map(Message::Echo)
    }

    fn view(&self) -> Element<Message> {
        let url_input = {
            let mut input = text_input("Type a url...", &self.url, Message::Url).padding(10);

            let mut start_button = button(
                text("Connect")
                    .height(Length::Fill)
                    .vertical_alignment(alignment::Vertical::Center),
            )
            .padding([0, 20]);

            let mut stop_button = button(
                text("Stop")
                    .height(Length::Fill)
                    .vertical_alignment(alignment::Vertical::Center),
            )
            .padding([0, 20]);

            if matches!(self.state, State::Reconnect(_)) {
                if let Some(message) = echo::Message::new(&self.url) {
                    input = input.on_submit(Message::Url(message.to_string()));
                    start_button = start_button.on_press(Message::Connect);
                }
            }
            if matches!(self.state, State::Connected(_)) {
                if let Some(_) = echo::Message::new("stop") {
                    stop_button = stop_button.on_press(Message::Stop);
                }
            }

            row![input, start_button, stop_button]
                .spacing(10)
                .align_items(Alignment::Fill)
        };

        let message_log: Element<_> = if self.messages.is_empty() {
            container(
                text("Your messages will appear here...").style(Color::from_rgb8(0x88, 0x88, 0x88)),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
        } else {
            scrollable(
                Column::with_children(
                    self.messages
                        .iter()
                        .cloned()
                        .map(text)
                        .map(Element::from)
                        .collect(),
                )
                .width(Length::Fill)
                .spacing(10),
            )
            .id(MESSAGE_LOG.clone())
            .height(Length::Fill)
            .into()
        };

        let new_message_input = {
            let mut input = text_input(
                "Type a message...",
                &self.new_message,
                Message::NewMessageChanged,
            )
            .padding(10);

            let mut button = button(
                text("Send")
                    .height(Length::Fill)
                    .vertical_alignment(alignment::Vertical::Center),
            )
            .padding([0, 20]);

            if matches!(self.state, State::Connected(_)) {
                if let Some(message) = echo::Message::new(&self.new_message) {
                    input = input.on_submit(Message::Send(message.clone()));
                    button = button.on_press(Message::Send(message));
                }
            }

            row![input, button].spacing(10).align_items(Alignment::Fill)
        };

        column![url_input, message_log, new_message_input]
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(20)
            .spacing(10)
            .into()
    }
}

enum State {
    Disconnected,
    Connected(echo::Connection),
    Reconnect(echo::Connection),
}

impl Default for State {
    fn default() -> Self {
        Self::Disconnected
    }
}

static MESSAGE_LOG: Lazy<scrollable::Id> = Lazy::new(scrollable::Id::unique);
