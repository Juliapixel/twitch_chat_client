use std::{
    fmt::Display,
    sync::{Arc, Weak},
};

use futures::future::BoxFuture;
use iced::{
    Border, Color, Element,
    widget::{Container, Text, column, container, tooltip},
};

use crate::widget::animated::AnimatedImage;

pub mod seventv;
pub mod twitch;

pub static DECODER_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

type MaybeImage = anyhow::Result<AnimatedImage>;

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

#[derive(Debug, Clone)]
pub struct ChannelEmote {
    pub image: Arc<async_once_cell::Lazy<MaybeImage, BoxFuture<'static, MaybeImage>>>,
    pub alias: Option<String>,
    pub metadata: Arc<EmoteMetadata>,
}

impl ChannelEmote {
    pub fn text_name(&self) -> &str {
        self.alias
            .as_deref()
            .unwrap_or(self.metadata.original_name.as_str())
    }

    pub fn view<M: 'static>(&self) -> Element<'static, M> {
        if let Some(image) = self.image.try_get().and_then(|i| i.as_ref().ok()) {
            tooltip(
                image.clone(),
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
            .into()
        } else {
            Text::new(self.text_name().to_owned())
                .color([0.8; 3])
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
