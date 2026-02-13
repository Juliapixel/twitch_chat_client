use std::{fmt::Display, sync::Arc, time::Duration};

use async_once_cell::Lazy;
use futures::future::BoxFuture;
use hashbrown::HashMap;
use iced::Length;
use moka::policy::EvictionPolicy;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::{
    platform::{
        ChannelEmote, DECODER_SEMAPHORE, EmoteFlags, EmoteImages, EmoteMetadata, MaybeImage,
    },
    util::default_client,
    widget::animated::AnimatedImage,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BttvUserQuery {
    shared_emotes: Vec<Emote>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Emote {
    code: String,
    code_original: Option<String>,
    id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EmoteSize {
    OneX,
    TwoX,
    ThreeX,
}

impl EmoteSize {
    fn size(&self) -> u32 {
        match self {
            EmoteSize::OneX => 28,
            EmoteSize::TwoX => 56,
            EmoteSize::ThreeX => 112,
        }
    }

    fn uniform_size(&self) -> Length {
        match self {
            EmoteSize::OneX => self.size().into(),
            EmoteSize::TwoX => self.size().into(),
            EmoteSize::ThreeX => self.size().into(),
        }
    }
}

impl Display for EmoteSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EmoteSize::OneX => "1x.webp",
            EmoteSize::TwoX => "2x.webp",
            EmoteSize::ThreeX => "3x.webp",
        })
    }
}

type EmoteCache = moka::future::Cache<(String, EmoteSize), MaybeImage>;

pub struct BetterTtvClient {
    client: reqwest::Client,
    channels: RwLock<HashMap<String, anyhow::Result<Arc<[ChannelEmote]>>>>,
    emotes: EmoteCache,
}

impl BetterTtvClient {
    pub fn new() -> Self {
        let cache = moka::future::CacheBuilder::new(300)
            .eviction_policy(EvictionPolicy::tiny_lfu())
            .time_to_idle(Duration::from_secs(60 * 30))
            .name("betterttv_emotes")
            .build();
        Self {
            client: default_client(),
            channels: Default::default(),
            emotes: cache,
        }
    }

    /// Blocks until the channel cache lock has been free'd
    pub fn channel_emote_set(&self, id: &str) -> Option<Arc<[ChannelEmote]>> {
        self.channels
            .blocking_read()
            .get(id)
            .and_then(|c| c.as_ref().ok())
            .cloned()
    }

    fn lazy_emote(
        &self,
        id: String,
        size: EmoteSize,
    ) -> Lazy<MaybeImage, BoxFuture<'static, MaybeImage>> {
        let client = self.client.clone();
        let cache = self.emotes.clone();

        Lazy::new(Box::pin(async move {
            cache
                .get_with((id.clone(), size), async move {
                    let start = std::time::Instant::now();
                    let data = client
                        .get(format!("https://cdn.betterttv.net/emote/{}/{size}", &id))
                        .header("Accept", "image/webp,image/png,image/gif")
                        .send()
                        .await
                        .inspect_err(|e| log::error!("{e}"))
                        .ok()?
                        .error_for_status()
                        .inspect_err(|e| log::error!("{e}"))
                        .ok()?
                        .bytes()
                        .await
                        .inspect_err(|e| log::error!("{e}"))
                        .ok()?;

                    let kbps = data.len() as f32 / 1000.0 / start.elapsed().as_secs_f32();

                    let img = {
                        let _ = DECODER_SEMAPHORE.acquire().await.unwrap();
                        tokio::task::spawn_blocking(move || AnimatedImage::from_bytes(&data))
                            .await
                            .inspect_err(|e| log::error!("{e}"))
                            .ok()?
                            .inspect_err(|e| log::error!("{e}"))
                            .ok()?
                    }
                    .width(size.uniform_size())
                    .height(size.uniform_size());

                    log::trace!(
                        "BTTV emote {id} loaded in {:?} at {kbps:02}kb/s",
                        start.elapsed()
                    );

                    Some(img)
                })
                .await
        }))
    }

    pub async fn load_channel_emote_set(&self, id: String) -> bool {
        let load = async || {
            let req = self
                .client
                .get(format!(
                    "https://api.betterttv.net/3/cached/users/twitch/{id}"
                ))
                .send()
                .await?
                .error_for_status()?
                .json::<BttvUserQuery>()
                .await?;

            let mut emotes = req
                .shared_emotes
                .into_iter()
                .map(|e| ChannelEmote {
                    images: Arc::new(EmoteImages {
                        one_x: (
                            self.lazy_emote(e.id.clone(), EmoteSize::OneX),
                            (EmoteSize::OneX.size(), EmoteSize::OneX.size()),
                        ),
                        two_x: Some((
                            self.lazy_emote(e.id.clone(), EmoteSize::TwoX),
                            (EmoteSize::TwoX.size(), EmoteSize::TwoX.size()),
                        )),
                        three_x: Some((
                            self.lazy_emote(e.id.clone(), EmoteSize::ThreeX),
                            (EmoteSize::ThreeX.size(), EmoteSize::ThreeX.size()),
                        )),
                        four_x: None,
                    }),
                    metadata: Arc::new(EmoteMetadata {
                        original_name: e.code_original.unwrap_or_else(|| e.code.clone()),
                        flags: EmoteFlags::empty(),
                        id: e.id,
                        platform: crate::platform::EmotePlatform::BetterTtv,
                    }),
                    alias: Some(e.code),
                })
                .collect::<Vec<_>>();

            emotes.sort_unstable_by(|a, b| a.text_name().cmp(b.text_name()));

            Ok(emotes.into())
        };

        let emotes = load().await;
        let loaded = emotes.is_ok();
        if let Err(e) = &emotes {
            log::error!("{e}");
        }

        let mut channels = self.channels.write().await;
        channels.insert(id.clone(), emotes);
        loaded
    }
}
