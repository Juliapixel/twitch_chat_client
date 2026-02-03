use std::{collections::VecDeque, ops::RangeInclusive, sync::Arc};

use futures::future::BoxFuture;
use hashbrown::HashMap;
use iced::{
    Alignment, Border, Color, Element, Length, Padding, Task,
    advanced::widget,
    alignment, mouse,
    widget::{
        Container, Row, Text, button, column, container, lazy, mouse_area, row, rule, space,
        text::{Rich, Span},
        text_input,
    },
};
use palette::{FromColor, IntoColor};
use twixel_core::irc_message::{AnySemantic, PrivMsg, tags::OwnedTag};

use crate::{
    IMAGE_GENERATION,
    platform::{
        ChannelEmote,
        twitch::{self, badges::BADGE_CACHE},
    },
    widget::{
        animated::AnimatedImage,
        scrollie::{ScrollViewport, scrollie},
    },
};

#[derive(Debug, Clone)]
pub struct Chat {
    pub channel: String,
    scroll_id: widget::Id,
    pub messages: VecDeque<(Arc<PrivMsg>, u64)>,
    pub message: String,
    pub usercard: Option<String>,

    emote_sets_loaded: bool,
    emote_generation: u64,
    pub emotes: HashMap<String, ChannelEmote>,

    show_scroll_to_bottom: bool,
}

#[allow(clippy::enum_variant_names)]
#[derive(derive_more::Debug)]
pub enum Message {
    SendMessage,
    MessageChange(String),
    CloseUserCard,
    ShowUserCard(String),
    ScrollToBottom,
    ChatScrolled(ScrollViewport),
    #[debug("Box<dyn CloneFn + Send>")]
    LoadImage(Box<dyn CloneFn + Send>),
    EmoteSetsLoaded,
    EmoteLoaded,
}

impl Clone for Message {
    fn clone(&self) -> Self {
        match self {
            Self::SendMessage => Self::SendMessage,
            Self::MessageChange(arg0) => Self::MessageChange(arg0.clone()),
            Self::CloseUserCard => Self::CloseUserCard,
            Self::ShowUserCard(arg0) => Self::ShowUserCard(arg0.clone()),
            Self::ScrollToBottom => Self::ScrollToBottom,
            Self::ChatScrolled(arg0) => Self::ChatScrolled(arg0.clone()),
            Self::LoadImage(arg0) => Self::LoadImage(arg0.clone_boxed()),
            Self::EmoteSetsLoaded => Self::EmoteSetsLoaded,
            Self::EmoteLoaded => Self::EmoteLoaded,
        }
    }
}

pub trait CloneFn: Fn() -> Task<Message> {
    fn clone_boxed(&self) -> Box<dyn CloneFn + Send>;
}

impl<T: Fn() -> Task<Message> + Clone + Send + 'static> CloneFn for T {
    fn clone_boxed(&self) -> Box<dyn CloneFn + Send> {
        Box::new(self.clone())
    }
}

impl Chat {
    pub fn new(channel: String) -> Self {
        Self {
            channel,
            scroll_id: widget::Id::unique(),
            messages: Default::default(),
            message: Default::default(),
            usercard: Default::default(),

            emote_sets_loaded: false,
            emote_generation: 0,
            emotes: Default::default(),

            show_scroll_to_bottom: false,
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let msgs = &self.messages;

        let header = row([
            button("hai").into(),
            space().width(Length::Fill).into(),
            self.channel.as_str().into(),
            space().width(Length::Fill).into(),
            button("hoi").into(),
        ])
        .width(Length::Fill)
        .align_y(alignment::Vertical::Center);

        let message_box = text_input(&format!("Send message in {}", &self.channel), &self.message)
            .on_paste(Message::MessageChange)
            .on_input(Message::MessageChange)
            .on_submit_maybe(if !self.message.trim().is_empty() {
                Some(Message::SendMessage)
            } else {
                None
            });

        let image_gen = IMAGE_GENERATION.load(std::sync::atomic::Ordering::Relaxed);

        column![
            header,
            rule::horizontal(1).style(rule::weak),
            iced::widget::stack!(
                scrollie(msgs.iter().map(|(m, key)| {
                    (
                        lazy(
                            (
                                key,
                                self.emote_generation,
                                self.emote_sets_loaded,
                                image_gen,
                            ),
                            |_| self.view_message(m),
                        ),
                        *key,
                    )
                }))
                .on_scroll(Message::ChatScrolled)
                .width(Length::Fill)
                .height(Length::Fill)
                .id(self.scroll_id.clone()),
                if self.show_scroll_to_bottom {
                    scroll_to_bottom()
                } else {
                    space().into()
                }
            ),
            message_box
        ]
        .into()
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SendMessage => {
                self.message.clear();
            }
            Message::MessageChange(m) => self.message = m,
            Message::ShowUserCard(user) => self.usercard = Some(user),
            Message::CloseUserCard => self.usercard = None,
            Message::ScrollToBottom => {
                return iced::widget::operation::snap_to_end(self.scroll_id.clone());
            }
            Message::ChatScrolled(vp) => {
                self.show_scroll_to_bottom = !vp.is_at_bottom();
            }
            Message::LoadImage(t) => return t().chain(Task::done(Message::EmoteLoaded)),
            Message::EmoteSetsLoaded => self.emote_sets_loaded = true,
            Message::EmoteLoaded => self.emote_generation += 1,
        };
        Task::none()
    }

    fn view_message(&self, msg: &PrivMsg) -> Element<'static, Message> {
        let badges = msg
            .badges()
            .filter_map(|(set, id)| {
                BADGE_CACHE
                    .get(&(set.to_owned(), id.to_owned()))
                    .and_then(|h| h.get()?.as_ref().ok().cloned())
            })
            .map(|h| Element::new(iced::widget::image(h.to_owned())))
            .collect::<Row<Message>>()
            .spacing(3);

        let emotes = msg
            .emotes()
            .filter_map(|(e, ranges)| {
                Some((
                    twitch::emotes::EMOTE_CACHE
                        .get(e)
                        .and_then(|h| h.get()?.as_ref().ok().cloned())?,
                    ranges,
                ))
            })
            .map(|(h, r)| (h.to_owned(), r))
            .collect::<Vec<(AnimatedImage, Vec<RangeInclusive<usize>>)>>();

        let username = msg
            .get_tag(OwnedTag::DisplayName)
            .or_else(|| msg.get_username().map(Into::into))
            .unwrap_or("FUCK".into());

        let [r, g, b] = msg.get_color().unwrap_or([96; 3]);
        let mut hsl: palette::Hsl = palette::Srgb::new(r, g, b).into_format().into_color();
        hsl.lightness = hsl.lightness.max(0.3);
        let (r, g, b) = palette::Srgb::from_color(hsl)
            .into_format()
            .into_components();
        let color = Color::from_rgb8(r, g, b);

        let mut char_pos = 0;
        let msg_col = if msg.is_me() { Some(color) } else { None };

        let spans = msg.message_text().split(' ').map(|w| {
            let word_chars = w.chars().count();
            let elem = emotes
                .iter()
                .find(|e| {
                    e.1.iter()
                        .any(|r| *r == (char_pos..=(char_pos + word_chars - 1)))
                })
                .map(|e| Element::new(e.0.clone()))
                .or_else(|| {
                    self.emotes
                        .get(w)
                        .map(|e| e.view().map(|t| Message::LoadImage(Box::new(t))))
                })
                .unwrap_or_else(|| Text::new(w.to_owned()).color_maybe(msg_col).into());
            char_pos += word_chars + 1;
            elem
        });

        let spans = itertools::intersperse_with(spans, || Text::new(" ").into());

        let text = Rich::<_, Message>::with_spans([
            Span::new(" "),
            Span::new(username.clone().into_owned())
                .color(color)
                .link(username.into_owned()),
            Span::new(": "),
        ])
        .on_link_click(Message::ShowUserCard);

        let line = [badges.into(), text.into()].into_iter().chain(spans);

        column![
            Container::new(Row::from_iter(line).align_y(Alignment::End).wrap())
                .padding(Padding::default().vertical(4.0).horizontal(6.0)),
            rule::horizontal(1),
        ]
        .into()
    }
}

fn scroll_to_bottom() -> Element<'static, Message> {
    container::Container::new(
        mouse_area(
            container::Container::new(Text::new("Scroll to Bottom"))
                .align_x(Alignment::Center)
                .padding(Padding::ZERO.vertical(4.0).horizontal(8.0))
                .style(|_| {
                    container::Style::default()
                        .background(Color::from_rgba8(0, 0, 0, 0.5))
                        .border(Border::default().rounded(6.0))
                }),
        )
        .on_press(Message::ScrollToBottom)
        .interaction(mouse::Interaction::Pointer),
    )
    .align_bottom(Length::Fill)
    .align_x(Alignment::Center)
    .width(Length::Fill)
    .padding(Padding::ZERO.bottom(8.0))
    .into()
}

fn view_irc(msg: &AnySemantic) -> Option<Element<'_, Message>> {
    match msg {
        AnySemantic::Pass(_) => None,
        AnySemantic::Nick(_) => None,
        AnySemantic::Join(join) => {
            let chan = join.get_param(0)?;
            Some(Rich::<(), _>::with_spans([Span::new("Joined "), Span::new(chan)]).into())
        }
        AnySemantic::Part(part) => {
            let chan = part.get_param(0)?;
            Some(Rich::<(), _>::with_spans([Span::new("Parted "), Span::new(chan)]).into())
        }
        AnySemantic::Notice(notice) => todo!(),
        AnySemantic::ClearMsg(clear_msg) => None,
        AnySemantic::ClearChat(clear_chat) => {
            let target = clear_chat.target_login()?;
            let duration = clear_chat.duration();
            match duration {
                twixel_core::irc_message::clearchat::TimeoutDuration::Permanent => Some(
                    Rich::<(), _>::with_spans([
                        Span::new("@"),
                        Span::new(target),
                        Span::new("was permanently banned"),
                    ])
                    .into(),
                ),
                twixel_core::irc_message::clearchat::TimeoutDuration::Temporary(duration) => Some(
                    Rich::<(), _>::with_spans([
                        Span::new("@"),
                        Span::new(target),
                        Span::new("was timed out for"),
                        Span::new(duration.as_secs().to_string()),
                        Span::new("s"),
                    ])
                    .into(),
                ),
            }
        }
        AnySemantic::HostTarget(_) => None,
        AnySemantic::PrivMsg(priv_msg) => Some(todo!()),
        AnySemantic::Ping(_) => {
            if cfg!(debug_assertions) {
                Some(Rich::<(), _>::with_spans([Span::new("Ping received from twitch.")]).into())
            } else {
                None
            }
        }
        AnySemantic::Pong(_) => None,
        AnySemantic::Cap(_) => None,
        AnySemantic::GlobalUserState(global_user_state) => todo!(),
        AnySemantic::UserState(user_state) => todo!(),
        AnySemantic::RoomState(room_state) => todo!(),
        AnySemantic::UserNotice(user_notice) => todo!(),
        AnySemantic::Reconnect(_) => Some(
            Rich::<(), _>::with_spans([Span::new(
                "Twitch has requested us to reconnect. Reconnecting...",
            )])
            .into(),
        ),
        AnySemantic::Whisper(_) => todo!(),
        AnySemantic::UnsupportedError(_) => None,
        AnySemantic::UserList(_) => None,
        AnySemantic::AuthSuccessful(_) => {
            Some(Rich::<(), _>::with_spans([Span::new("Connected.")]).into())
        }
        AnySemantic::Useless(_) => None,
    }
}
