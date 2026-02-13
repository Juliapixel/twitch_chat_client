use std::{fmt::Display, sync::Arc, time::Duration};

use async_once_cell::Lazy;
use futures::future::BoxFuture;
use hashbrown::HashMap;
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
struct FfzRoomQuery {
    sets: std::collections::HashMap<String, EmoteSet>,
}

#[derive(Debug, Deserialize)]
struct EmoteSet {
    emoticons: Vec<Emote>,
}

#[derive(Debug, Deserialize)]
struct Emote {
    name: String,
    width: u32,
    height: u32,
    animated: Option<serde::de::IgnoredAny>,
    id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EmoteSize {
    OneX,
    TwoX,
    FourX,
}

impl Display for EmoteSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EmoteSize::OneX => "1",
            EmoteSize::TwoX => "2",
            EmoteSize::FourX => "3",
        })
    }
}

type EmoteCache = moka::future::Cache<(i64, EmoteSize), MaybeImage>;

pub struct FfzClient {
    client: reqwest::Client,
    channels: RwLock<HashMap<String, anyhow::Result<Arc<[ChannelEmote]>>>>,
    emotes: EmoteCache,
}

impl FfzClient {
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
    pub fn channel_emote_set_login(&self, login: &str) -> Option<Arc<[ChannelEmote]>> {
        self.channels
            .blocking_read()
            .get(login)
            .and_then(|c| c.as_ref().ok())
            .cloned()
    }

    fn lazy_emote(
        &self,
        id: i64,
        size: EmoteSize,
        animated: bool,
    ) -> Lazy<MaybeImage, BoxFuture<'static, MaybeImage>> {
        let client = self.client.clone();
        let cache = self.emotes.clone();

        Lazy::new(Box::pin(async move {
            cache
                .get_with((id, size), async move {
                    let url = if animated {
                        format!("https://cdn.frankerfacez.com/emoticon/{id}/animated/{size}")
                    } else {
                        format!("https://cdn.frankerfacez.com/emoticon/{id}/{size}")
                    };
                    let start = std::time::Instant::now();
                    let data = client
                        .get(url)
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
                    };

                    log::trace!(
                        "BTTV emote {id} loaded in {:?} at {kbps:02}kb/s",
                        start.elapsed()
                    );

                    Some(img)
                })
                .await
        }))
    }

    pub async fn load_channel_emote_set_login(&self, login: String) -> bool {
        let load = async || {
            let req = self
                .client
                .get(format!("https://api.frankerfacez.com/v1/room/{login}"))
                .send()
                .await?
                .error_for_status()?
                .json::<FfzRoomQuery>()
                .await?;

            let mut emotes = req
                .sets
                .into_values()
                .flat_map(|s| s.emoticons.into_iter())
                .map(|e| {
                    let animated = e.animated.is_some();
                    ChannelEmote {
                        images: Arc::new(EmoteImages {
                            one_x: (
                                self.lazy_emote(e.id, EmoteSize::OneX, animated),
                                (e.width, e.height),
                            ),
                            two_x: Some((
                                self.lazy_emote(e.id, EmoteSize::TwoX, animated),
                                (e.width * 2, e.height * 2),
                            )),
                            three_x: None,
                            four_x: Some((
                                self.lazy_emote(e.id, EmoteSize::FourX, animated),
                                (e.width * 4, e.width * 4),
                            )),
                        }),
                        metadata: Arc::new(EmoteMetadata {
                            original_name: e.name,
                            flags: EmoteFlags::empty(),
                            id: e.id.to_string(),
                            platform: crate::platform::EmotePlatform::FrankerFaceZ,
                        }),
                        alias: None,
                    }
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
        channels.insert(login, emotes);
        loaded
    }
}
