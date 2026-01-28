use std::{borrow::Cow, str::FromStr, sync::Arc, time::Duration};

use graphql_client::GraphQLQuery;
use hashbrown::HashMap;
use moka::policy::EvictionPolicy;
use serde::{Deserialize, de::Visitor};
use tokio::sync::OnceCell;
use ulid::Ulid;

type Id = Ulid;

#[derive(graphql_client::GraphQLQuery)]
#[graphql(
    schema_path = "schemas/seventv.json",
    query_path = "src/platform/seventv/emotes_by_twitch_id.graphql"
)]
struct GetEmoteSet;

#[derive(Deserialize)]
struct SevenTvUserQuery<'a> {
    id: String,
    #[serde(borrow)]
    emote_set: EmoteSet<'a>,
}

#[derive(Deserialize)]
struct EmoteSet<'a> {
    #[serde(borrow)]
    emotes: Vec<Emote<'a>>,
}

#[derive(Deserialize)]
struct Emote<'a> {
    id: ulid::Ulid,
    #[serde(borrow)]
    name: Cow<'a, str>,
    host: EmoteHost<'a>,
}

#[derive(Deserialize)]
struct EmoteHost<'a> {
    #[serde(borrow)]
    files: Vec<File<'a>>,
}

#[derive(Deserialize)]
struct File<'a> {
    #[serde(borrow)]
    static_name: Cow<'a, str>,
}

#[derive(Clone)]
struct SevenTvEmote {
    max_size: MaxSize,
    name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MaxSize {
    OneX,
    TwoX,
    ThreeX,
    FourX,
}

impl FromStr for MaxSize {
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

pub struct SevenTv {
    client: reqwest::Client,
    channels: moka::sync::Cache<String, Arc<OnceCell<anyhow::Result<Vec<SevenTvEmote>>>>>,
}

impl SevenTv {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            channels: moka::sync::CacheBuilder::new(300)
                .eviction_policy(EvictionPolicy::tiny_lfu())
                .time_to_idle(Duration::from_secs(60 * 30))
                .name("seventv emote sets")
                .build(),
        }
    }

    /*
    pub async fn channel_emote_set(&mut self, id: String) -> Result<&Vec<SevenTvEmote>, &anyhow::Error> {
        let query = GetEmoteSet::build_query(get_emote_set::Variables { id });

        let cell = self.channels.get_with_by_ref(&id, || {
            Arc::new(OnceCell::new())
        });

        cell.get_or_init(async || {
            let req = self.client.get(format!("https://7tv.io/v3/users/twitch/{id}"))
                .send()
                .await?
                .error_for_status()?
                .json::<SevenTvUserQuery>()
                .await?;

            let emotes = req.emote_set.emotes.into_iter().map(|e| {
                let max_size = e.host.files.into_iter().fold(MaxSize::OneX, |acc, h| h.static_name.parse::<MaxSize>().map(|m| m.max(acc)).unwrap_or(acc));
                SevenTvEmote { name: e.name.into(), max_size }}
            ).collect();

            Ok(emotes)
        }).await;

        cell
    }
    */
}
