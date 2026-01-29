use std::{
    fmt::Display,
    str::FromStr,
    sync::{Arc, LazyLock},
    time::Duration,
};

use hashbrown::HashMap;
use moka::policy::EvictionPolicy;
use serde::Deserialize;
use tokio::sync::{OnceCell, RwLock};
use ulid::Ulid;

use crate::{platform::DECODER_SEMAPHORE, widget::animated::AnimatedImage};

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

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
    name: String,
    host: Option<EmoteHost>,
}

#[derive(Deserialize)]
struct EmoteHost {
    files: Vec<File>,
}

#[derive(Deserialize)]
struct File {
    static_name: String,
}

#[derive(Clone)]
pub struct SevenTvEmote {
    pub max_size: EmoteSize,
    pub alias: String,
    pub id: Ulid,
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

pub static CHANNELS: LazyLock<RwLock<HashMap<String, anyhow::Result<Vec<SevenTvEmote>>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn load_channel_emote_set(id: String) -> bool {
    let mut channels = CHANNELS.write().await;
    let entry = channels.entry_ref(&id);

    match entry {
        hashbrown::hash_map::EntryRef::Vacant(e) => {
            let func = async || {
                let req = CLIENT
                    .get(format!("https://7tv.io/v3/users/twitch/{id}"))
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<SevenTvUserQuery>()
                    .await?;

                let emotes = req
                    .emote_set
                    .emotes
                    .into_iter()
                    .map(|e| {
                        let max_size = e
                            .host
                            .map(|h| h.files.into_iter())
                            .into_iter()
                            .flatten()
                            .fold(EmoteSize::OneX, |acc, h| {
                                h.static_name
                                    .parse::<EmoteSize>()
                                    .map(|m| m.max(acc))
                                    .unwrap_or(acc)
                            });
                        SevenTvEmote {
                            alias: e.name,
                            max_size,
                            id: e.id,
                        }
                    })
                    .collect();

                Ok(emotes)
            };

            let value = func().await;

            if let Err(e) = value {
                log::warn!("failed to load 7tv channel data for {id}: {e:?}");
            }

            e.insert(func().await);
            true
        }
        _ => false,
    }
}

type EmoteCache =
    moka::sync::Cache<(Ulid, EmoteSize), Arc<OnceCell<anyhow::Result<AnimatedImage>>>>;

pub static EMOTE_CACHE: LazyLock<EmoteCache> = LazyLock::new(|| {
    moka::sync::CacheBuilder::new(300)
        .eviction_policy(EvictionPolicy::tiny_lfu())
        .time_to_idle(Duration::from_secs(60 * 30))
        .name("badges")
        .build()
});

pub async fn load_emote(id: Ulid, size: EmoteSize) -> bool {
    let mut loaded = false;

    let entry = EMOTE_CACHE.get_with((id, size), || Arc::new(tokio::sync::OnceCell::new()));

    let emote = entry
        .get_or_init(async || {
            let data = CLIENT
                .get(format!("https://cdn.7tv.app/emote/{id}/{size}"))
                .header("Accept", "image/webp,image/png,image/gif")
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;

            let img = {
                let _ = DECODER_SEMAPHORE.acquire().await.unwrap();
                tokio::task::spawn_blocking(move || AnimatedImage::from_bytes(&data)).await??
            };

            loaded = true;

            Ok(img)
        })
        .await;

    if let Err(e) = emote {
        log::warn!("failed to load 7tv emote: {e}")
    };

    loaded
}
