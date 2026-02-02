use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use futures::{SinkExt, Stream, StreamExt, TryFutureExt, channel::mpsc::UnboundedSender};
use iced::{
    Alignment, Color, Element, Length, Subscription, Task, Theme, stream,
    widget::{container, opaque, space},
    window,
};
use indexmap::IndexMap;
use twixel_core::{
    MessageBuilder,
    auth::Anonymous,
    irc_message::{AnySemantic, PrivMsg, SemanticIrcMessage},
};

use crate::{
    chat::Chat,
    components::join_popup::{self, JoinPopup},
    config::CONFIG,
    config_ui::ConfigUi,
    operation::switch_to_tab,
    platform::{
        seventv::{self, SevenTvClient},
        twitch::{self, badges::load_badge},
    },
    title_bar::TitleBar,
    widget::tabs::Tabs,
};

mod chat;
mod cli;
mod components;
mod config;
mod config_ui;
mod operation;
mod platform;
mod title_bar;
mod util;
mod widget;

static MESSAGE_KEY: AtomicU64 = AtomicU64::new(1);

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

enum IrcCommand {
    Join(String),
    Part(String),
    Message(String, String),
}

struct Juliarino {
    tabs_id: iced::widget::Id,
    irc_command: Option<UnboundedSender<IrcCommand>>,

    seventv_client: Arc<SevenTvClient>,

    join_window: Option<JoinPopup>,
    channels: IndexMap<String, Chat>,
    config: ConfigUi,
    title_bar: TitleBar,
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug)]
enum Message {
    /// A new image was loaded into cache (emote or badge)
    ImageLoaded,
    ChannelSevenTvDataLoaded {
        login: String,
        id: String,
    },
    IrcConnected(UnboundedSender<IrcCommand>),
    /// Close button on a tab was closed
    TabClosed(String),
    /// A tab open request was made for the given channel
    OpenJoin,
    CloseJoin,
    /// Should add this tab
    OpenTab(String),
    /// A channel has been successfully joined via IRC
    ChannelJoined(String),
    /// New message received over IRC
    NewMessage(PrivMsg),
    /// Message for [components::join_popup::JoinPopup]
    JoinPopupMessage(join_popup::Message),
    /// Message for [chat::Chat]
    ChatMessage(String, chat::Message),
    /// Message for [config_ui::ConfigUi]
    ConfigMessage(config_ui::Message),
    /// Message for [title_bar::TitleBar]
    TitleBarMessage(title_bar::Message),
}

static IMAGE_GENERATION: AtomicU64 = AtomicU64::new(0);

impl Juliarino {
    fn new(channels: impl IntoIterator<Item = impl Into<String>>, main_window: window::Id) -> Self {
        let chats: IndexMap<String, Chat> = channels
            .into_iter()
            .map(|c| {
                let c = c.into();
                (c.clone(), Chat::new(c))
            })
            .collect();
        Self {
            tabs_id: iced::widget::Id::unique(),
            join_window: None,
            seventv_client: Arc::new(SevenTvClient::new()),
            channels: chats,
            config: ConfigUi::new(),
            irc_command: None,
            title_bar: TitleBar::new("Juliarino", main_window),
        }
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::IrcConnected(tx) => self.irc_command = Some(tx),
            Message::NewMessage(priv_msg) => {
                let chan = priv_msg.channel_login();
                let Some(chat) = self.channels.get_mut(chan) else {
                    return Task::none();
                };

                let seventv_emotes = if let Some(id) = priv_msg.channel_id() {
                    self.seventv_client.try_channel_emote_set(id)
                } else {
                    None
                }
                .unwrap_or_default();

                let p = &priv_msg;

                let seventv_emotes_task = p
                    .message_text()
                    .split(' ')
                    .filter_map(|w| {
                        seventv_emotes
                            .binary_search_by(|e| e.text_name().cmp(w))
                            .ok()
                            .map(|i| seventv_emotes[i].image.clone())
                    })
                    .filter(|i| i.try_get().is_none())
                    .map(|e| Task::future(async move { e.get_unpin().await.is_ok() }));

                let badge_tasks = priv_msg
                    .badges()
                    .map(|(set, id)| (set.to_owned(), id.to_owned()))
                    .map(|(set, id)| Task::future(async { load_badge(set, id).await }));

                let emote_tasks = priv_msg
                    .emotes()
                    .map(|e| Task::future(twitch::emotes::load_emote(e.0.to_owned())));

                if chat.messages.len() > 500 {
                    chat.messages.pop_front();
                }
                let task = Task::batch(badge_tasks.chain(emote_tasks).chain(seventv_emotes_task))
                    .then(|r| {
                        if r {
                            Task::done(Message::ImageLoaded)
                        } else {
                            Task::none()
                        }
                    });
                let key = MESSAGE_KEY.fetch_add(1, Ordering::Relaxed);
                chat.messages.push_back((Arc::new(priv_msg), key));
                return task;
            }
            Message::TabClosed(tab) => {
                let mut config = CONFIG.write();
                config.chats.retain(|c| c != &tab);
                config.save().unwrap();
                drop(config);

                self.channels.shift_remove(&tab);
                if let Some(tx) = &self.irc_command {
                    tx.unbounded_send(IrcCommand::Part(tab)).unwrap();
                }
            }
            Message::OpenJoin => {
                self.join_window = Some(JoinPopup::new());
            }
            Message::CloseJoin => {
                self.join_window = None;
            }
            Message::OpenTab(tab) => {
                let tab = tab.to_lowercase().trim().to_owned();
                let mut config = CONFIG.write();
                config.chats.push(tab.clone());
                config.save().unwrap();
                drop(config);

                self.channels.insert(tab.clone(), Chat::new(tab.clone()));
                if let Some(tx) = &self.irc_command {
                    tx.unbounded_send(IrcCommand::Join(tab.clone())).unwrap();
                }
                self.join_window = None;
                return switch_to_tab(self.tabs_id.clone(), tab).discard();
            }
            Message::ChannelJoined(chan) => {
                let stv = self.seventv_client.clone();
                return Task::future(async move {
                    let data =
                        reqwest::get(format!("https://api.ivr.fi/v2/twitch/user?login={}", &chan))
                            .and_then(|r| r.json::<serde_json::Value>())
                            .inspect_err(|e| log::error!("{e}"))
                            .await;

                    if let Some(id) = data.ok().as_ref().and_then(|d| d[0]["id"].as_str()) {
                        (
                            chan,
                            id.to_owned(),
                            stv.load_channel_emote_set(id.to_owned()).await,
                        )
                    } else {
                        (chan, "".into(), false)
                    }
                })
                .then(|(c, id, f)| {
                    if f {
                        Task::done(Message::ChannelSevenTvDataLoaded { login: c, id })
                    } else {
                        Task::none()
                    }
                });
            }
            Message::JoinPopupMessage(m) => {
                if let Some(p) = &mut self.join_window {
                    return p.update(m).discard();
                }
            }
            Message::ChatMessage(chat, msg) => {
                let Some(chat_elem) = self.channels.get_mut(&chat) else {
                    return Task::none();
                };
                if matches!(msg, chat::Message::SendMessage)
                    && let Some(tx) = &self.irc_command
                {
                    let _ = tx.unbounded_send(IrcCommand::Message(
                        chat_elem.channel.clone(),
                        chat_elem.message.clone(),
                    ));
                }
                return chat_elem
                    .update(msg)
                    .map(move |m| Message::ChatMessage(chat.clone(), m));
            }
            Message::ConfigMessage(msg) => {
                self.config.update(msg);
            }
            Message::TitleBarMessage(message) => return self.title_bar.update(message).discard(),
            Message::ChannelSevenTvDataLoaded { login, id } => {
                if let (Some(chan), Some(emotes)) = (
                    self.channels.get_mut(&login),
                    self.seventv_client.channel_emote_set(&id),
                ) {
                    chan.emotes
                        .extend(emotes.iter().map(|e| (e.text_name().to_owned(), e.clone())));
                }
            }
            // Signaling messages
            Message::ImageLoaded => {
                IMAGE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        };
        Task::none()
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        let tabs = self.channels.iter().map(|(c, chat)| {
            let span = iced::debug::time(format!("chat view ({c})"));
            let view = chat
                .view()
                .map(move |m| Message::ChatMessage(c.to_owned(), m));
            span.finish();
            (c.clone(), view)
        });
        let tabs = Tabs::new(tabs)
            .id(self.tabs_id.clone())
            .on_close(Message::TabClosed)
            .on_add(Message::OpenJoin)
            .fallback(self.config.view().map(Message::ConfigMessage));

        let popup: Element<'_, Message> = self
            .join_window
            .as_ref()
            .map(|w| {
                opaque(
                    container(w.view().map(|m| match m {
                        join_popup::Message::Submit => Message::OpenTab(w.value.clone()),
                        join_popup::Message::Close => Message::CloseJoin,
                        m => Message::JoinPopupMessage(m),
                    }))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(|_| {
                        container::Style::default().background(Color::BLACK.scale_alpha(0.3))
                    }),
                )
            })
            .unwrap_or_else(|| space().into());

        iced::widget::stack!(tabs, popup).into()
    }
}

fn twitch_worker() -> impl Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        output.send(Message::IrcConnected(tx)).await.unwrap();
        loop {
            let mut conn = twixel_core::Connection::new(CONFIG.read().chats.iter(), Anonymous {});
            conn.start().await.unwrap();
            loop {
                futures::select! {
                    msg = conn.next() => match msg.map(|m| m.map(AnySemantic::from)) {
                        Some(Ok(AnySemantic::PrivMsg(msg))) => {
                            output.send(Message::NewMessage(msg)).await.unwrap();
                        },
                        Some(Ok(AnySemantic::Ping(ping))) => {
                            conn.send(ping.respond().to_owned())
                                .await
                                .unwrap();
                        },
                        Some(Ok(AnySemantic::Join(join))) => {
                            let Some(chan) = join.get_param(0) else {
                                log::debug!("JOIN message without param??? {:?}", join.inner());
                                continue;
                            };

                            output.send(Message::ChannelJoined(chan.trim_start_matches('#').to_owned()))
                                .await
                                .unwrap();
                        },
                        Some(Ok(m)) => log::debug!("{}", m.inner().inner().trim()),
                        Some(Err(e)) => {
                            log::error!("{e}");
                            break;
                        }
                        None => {
                            log::warn!("IRC connection closed with no error");
                            break;
                        },
                    },
                    msg = rx.next() => match msg {
                        Some(IrcCommand::Part(chan)) => {
                            log::info!("Parting #{}", &chan);
                            conn.part(&chan).await.unwrap();
                        },
                        Some(IrcCommand::Join(chan)) => {
                            log::info!("Joining #{}", &chan);
                            conn.join(&chan).await.unwrap();
                        },
                        Some(IrcCommand::Message(chan, msg)) => {
                            log::info!("Sending \"{}\" to #{}", &msg, &chan);
                            conn.send(MessageBuilder::privmsg(&chan, &msg)).await.unwrap();
                        },
                        None => {
                            panic!("IRC control channel closed");
                        },
                    }
                }
            }
        }
    })
}

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(if cfg!(debug_assertions) {
            log::LevelFilter::Info
        } else {
            log::LevelFilter::Warn
        })
        .filter_module(
            "juliarino_iced",
            if cfg!(debug_assertions) {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            },
        )
        .parse_default_env()
        .init();
    log::info!("Logging started");

    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    iced::daemon(
        || {
            let (id, task) = iced::window::open(window::Settings {
                // decorations: false,
                ..Default::default()
            });
            (
                Juliarino::new(CONFIG.read().chats.iter(), id),
                task.discard(),
            )
        },
        Juliarino::update,
        Juliarino::view,
    )
    .subscription(|_| Subscription::run(twitch_worker))
    .theme(|_s: &Juliarino, _| Some(Theme::CatppuccinMacchiato))
    .title(if cfg!(debug_assertions) {
        concat!("Juliarino - ", env!("CARGO_PKG_VERSION"), " (DEBUG)")
    } else {
        concat!("Juliarino - ", env!("CARGO_PKG_VERSION"))
    })
    .run()?;
    Ok(())
}
