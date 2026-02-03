use std::{fmt::Display, str::FromStr, sync::Arc, time::Duration};

use async_once_cell::Lazy;
use futures::future::BoxFuture;
use hashbrown::HashMap;
use moka::policy::EvictionPolicy;
use serde::Deserialize;
use tokio::sync::RwLock;
use ulid::Ulid;

use crate::{
    platform::{ChannelEmote, DECODER_SEMAPHORE, EmoteFlags, EmoteMetadata},
    util::default_client,
    widget::animated::AnimatedImage,
};

#[cfg(feature = "unstable")]
mod eventapi;

#[cfg(feature = "unstable")]
pub use eventapi::EventApiClient;

type Id = Ulid;

#[derive(graphql_client::GraphQLQuery)]
#[graphql(
    schema_path = "schemas/seventv.json",
    query_path = "src/platform/seventv/emotes_by_twitch_id.graphql"
)]
struct GetEmoteSet;

#[derive(Deserialize)]
struct SevenTvUserQuery {
    emote_set: EmoteSet,
}

#[derive(Deserialize)]
struct EmoteSet {
    emotes: Vec<Emote>,
}

#[derive(Deserialize)]
struct Emote {
    id: ulid::Ulid,
    /// Alias in channel
    name: String,
    data: EmoteData,
}

#[derive(Deserialize)]
struct EmoteData {
    /// Original name
    name: String,
    flags: SevenTvEmoteFlags,
    host: EmoteHost,
}

#[derive(Deserialize)]
struct SevenTvEmoteFlags(u32);

// https://github.com/SevenTV/SevenTV/blob/a558d2c28d3f9e4feccf71ef32d7771384910b7f/shared/src/old_types/mod.rs#L622-L632
bitflags::bitflags! {
    impl SevenTvEmoteFlags: u32 {
        const PRIVATE = 1 << 0;
        const AUTHENTIC = 1 << 1;
        const ZERO_WIDTH = 1 << 8;
        const SEXUAL = 1 << 16;
        const EPILEPSY = 1 << 17;
        const EDGY = 1 << 18;
        const TWITCHDISALLOWED = 1 << 24;
    }
}

impl From<SevenTvEmoteFlags> for EmoteFlags {
    fn from(value: SevenTvEmoteFlags) -> Self {
        let mut flags = EmoteFlags::empty();
        if value.contains(SevenTvEmoteFlags::ZERO_WIDTH) {
            flags |= EmoteFlags::OVERLAYING;
        }
        if value.contains(SevenTvEmoteFlags::PRIVATE) {
            flags |= EmoteFlags::HIDDEN;
        }
        flags
    }
}

#[derive(Deserialize)]
struct EmoteHost {
    files: Vec<File>,
}

#[derive(Deserialize)]
struct File {
    static_name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EmoteSize {
    OneX,
    TwoX,
    ThreeX,
    FourX,
}

impl FromStr for EmoteSize {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.chars().next().ok_or(())? {
            '1' => Ok(Self::OneX),
            '2' => Ok(Self::TwoX),
            '3' => Ok(Self::ThreeX),
            '4' => Ok(Self::FourX),
            _ => Err(()),
        }
    }
}

impl Display for EmoteSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EmoteSize::OneX => "1x.webp",
            EmoteSize::TwoX => "2x.webp",
            EmoteSize::ThreeX => "3x.webp",
            EmoteSize::FourX => "4x.webp",
        })
    }
}

type MaybeImage = anyhow::Result<AnimatedImage>;

type EmoteCache =
    moka::sync::Cache<(Ulid, EmoteSize), Arc<Lazy<MaybeImage, BoxFuture<'static, MaybeImage>>>>;

pub struct SevenTvClient {
    client: reqwest::Client,
    channels: RwLock<HashMap<String, anyhow::Result<Arc<[ChannelEmote]>>>>,
    emotes: EmoteCache,
}

impl SevenTvClient {
    pub fn new() -> Self {
        let client = default_client();
        let cache = moka::sync::CacheBuilder::new(300)
            .eviction_policy(EvictionPolicy::tiny_lfu())
            .time_to_idle(Duration::from_secs(60 * 30))
            .name("badges")
            .build();
        Self {
            client,
            channels: Default::default(),
            emotes: cache,
        }
    }

    pub async fn get_globals(&self) -> anyhow::Result<Vec<ChannelEmote>> {
        let req = self
            .client
            .get("https://7tv.io/v3/emote-sets/global")
            .send()
            .await?
            .error_for_status()?
            .json::<EmoteSet>()
            .await?;

        let mut emotes = req
            .emotes
            .into_iter()
            .map(|e| {
                let max_size = e
                    .data
                    .host
                    .files
                    .into_iter()
                    .fold(EmoteSize::OneX, |acc, h| {
                        h.static_name
                            .parse::<EmoteSize>()
                            .map(|m| m.max(acc))
                            .unwrap_or(acc)
                    });
                ChannelEmote {
                    image: self.lazy_emote(e.id, EmoteSize::OneX),
                    alias: None,
                    metadata: Arc::new(EmoteMetadata {
                        original_name: e.name,
                        flags: e.data.flags.into(),
                        id: e.id.to_string(),
                        platform: crate::platform::EmotePlatform::SevenTv,
                    }),
                }
            })
            .collect::<Vec<_>>();

        emotes.sort_unstable_by(|a, b| a.text_name().cmp(b.text_name()));

        Ok(emotes)
    }

    /// Blocks until the channel cache lock has been free'd
    pub fn channel_emote_set(&self, id: &str) -> Option<Arc<[ChannelEmote]>> {
        self.channels
            .blocking_read()
            .get(id)
            .and_then(|c| c.as_ref().ok())
            .cloned()
    }

    /// Gets the channel's emote set without blocking
    pub fn try_channel_emote_set(&self, id: &str) -> Option<Arc<[ChannelEmote]>> {
        self.channels
            .try_read()
            .ok()?
            .get(id)
            .and_then(|c| c.as_ref().ok())
            .cloned()
    }

    fn lazy_emote(
        &self,
        id: Ulid,
        size: EmoteSize,
    ) -> Arc<Lazy<MaybeImage, BoxFuture<'static, MaybeImage>>> {
        let client = self.client.clone();
        self.emotes.get_with((id, size), || {
            Arc::new(Lazy::new(Box::pin(async move {
                let start = std::time::Instant::now();
                let data = client
                    .get(format!("https://cdn.7tv.app/emote/{id}/{size}"))
                    .header("Accept", "image/webp,image/png,image/gif")
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?;

                let kbps = data.len() as f32 / 1000.0 / start.elapsed().as_secs_f32();

                let img = {
                    let _ = DECODER_SEMAPHORE.acquire().await.unwrap();
                    tokio::task::spawn_blocking(move || AnimatedImage::from_bytes(&data)).await??
                };

                log::trace!(
                    "7TV emote {id} loaded in {:?} at {kbps:02}kb/s",
                    start.elapsed()
                );

                Ok(img)
            })))
        })
    }

    pub async fn load_channel_emote_set(&self, id: String) -> bool {
        let load = async || {
            let req = self
                .client
                .get(format!("https://7tv.io/v3/users/twitch/{id}"))
                .send()
                .await?
                .error_for_status()?
                .json::<SevenTvUserQuery>()
                .await?;

            let mut emotes = req
                .emote_set
                .emotes
                .into_iter()
                .map(|e| {
                    let max_size = e
                        .data
                        .host
                        .files
                        .into_iter()
                        .fold(EmoteSize::OneX, |acc, h| {
                            h.static_name
                                .parse::<EmoteSize>()
                                .map(|m| m.max(acc))
                                .unwrap_or(acc)
                        });
                    ChannelEmote {
                        image: self.lazy_emote(e.id, EmoteSize::OneX),
                        alias: Some(e.name),
                        metadata: Arc::new(EmoteMetadata {
                            original_name: e.data.name,
                            flags: e.data.flags.into(),
                            id: e.id.to_string(),
                            platform: crate::platform::EmotePlatform::SevenTv,
                        }),
                    }
                })
                .collect::<Vec<_>>();

            emotes.sort_unstable_by(|a, b| a.text_name().cmp(b.text_name()));

            Ok(emotes.into())
        };

        let emotes = load().await;
        let loaded = emotes.is_ok();

        let mut channels = self.channels.write().await;
        channels.insert(id.clone(), emotes);
        loaded
    }
}
