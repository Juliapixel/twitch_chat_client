use std::{fmt::Display, sync::Arc};

use async_once_cell::Lazy;
use futures::future::BoxFuture;
use iced::{
    Border, Color, Element, Task,
    widget::{Container, Space, Text, column, container, sensor, tooltip},
};

use crate::widget::animated::AnimatedImage;

pub mod recent_messages;
pub mod seventv;
pub mod twitch;

pub static DECODER_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

type MaybeImage = Option<AnimatedImage>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EmotePlatform {
    SevenTv,
    FrankerFaceZ,
    BetterTtv,
    Twitch,
}

impl EmotePlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            EmotePlatform::SevenTv => "7TV",
            EmotePlatform::FrankerFaceZ => "FFZ",
            EmotePlatform::BetterTtv => "BTTV",
            EmotePlatform::Twitch => "Twitch",
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct EmoteFlags: u16 {
        const OVERLAYING = 1;
        const HIDDEN = 1 << 1;
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct EmoteMetadata {
    pub original_name: String,
    pub flags: EmoteFlags,
    pub id: String,
    pub platform: EmotePlatform,
}

type EmoteImage = (Lazy<MaybeImage, BoxFuture<'static, MaybeImage>>, (u32, u32));

#[derive(Debug)]
pub struct EmoteImages {
    one_x: EmoteImage,
    two_x: Option<EmoteImage>,
    three_x: Option<EmoteImage>,
    four_x: Option<EmoteImage>,
}

#[derive(Debug, Clone)]
pub struct ChannelEmote {
    pub images: Arc<EmoteImages>,
    pub alias: Option<String>,
    pub metadata: Arc<EmoteMetadata>,
}

impl ChannelEmote {
    pub fn text_name(&self) -> &str {
        self.alias
            .as_deref()
            .unwrap_or(self.metadata.original_name.as_str())
    }

    pub fn view<M: Send + 'static>(
        &self,
    ) -> Element<'static, impl Fn() -> Task<M> + Clone + 'static> {
        let tooltiper = |e: Element<'static, _>| {
            tooltip(
                e,
                Container::new(column![
                    Text::new(self.text_name().to_owned()),
                    Text::new(self.metadata.platform.as_str())
                ])
                .padding(12)
                .style(|_| {
                    container::Style::default()
                        .border(Border::default().rounded(6.0))
                        .background(Color::from_rgba(0.0, 0.0, 0.0, 0.8))
                }),
                tooltip::Position::Top,
            )
        };

        if let Some(image) = self.images.one_x.0.try_get().and_then(|i| i.as_ref()) {
            tooltiper(image.clone().into()).into()
        } else {
            let copy = self.images.clone();
            let placeholder = Space::new()
                .width(self.images.one_x.1.0)
                .height(self.images.one_x.1.1);
            tooltiper(Element::new(sensor(placeholder).on_show(move |_| {
                let sent = copy.clone();
                move || {
                    let sent2 = sent.clone();
                    Task::future(async move {
                        sent2.one_x.0.get_unpin().await;
                    })
                    .discard()
                }
            })))
            .into()
        }
    }
}

impl Display for EmotePlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq for ChannelEmote {
    fn eq(&self, other: &Self) -> bool {
        self.alias.eq(&other.alias) && self.metadata.eq(&other.metadata)
    }
}

impl PartialOrd for ChannelEmote {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.alias.partial_cmp(&other.alias) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.metadata.partial_cmp(&other.metadata)
    }
}
