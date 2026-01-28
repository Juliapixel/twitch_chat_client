pub mod badges {
    use std::{
        borrow::Cow,
        collections::HashMap,
        sync::{Arc, LazyLock},
        time::Duration,
    };

    use iced::widget::image::Handle;
    use moka::policy::EvictionPolicy;
    use serde::Deserialize;
    use tokio::sync::OnceCell;

    use crate::res_str;

    #[derive(Deserialize)]
    struct BadgesCache {
        #[serde(borrow)]
        id: Cow<'static, str>,
        #[serde(borrow)]
        image: Cow<'static, str>,
    }

    type BadgeCache = moka::sync::Cache<(String, String), Arc<OnceCell<anyhow::Result<Handle>>>>;

    pub static BADGE_CACHE: LazyLock<BadgeCache> = LazyLock::new(|| {
        moka::sync::CacheBuilder::new(300)
            .eviction_policy(EvictionPolicy::tiny_lfu())
            .time_to_idle(Duration::from_secs(60 * 30))
            .name("badges")
            .build()
    });

    static SAVED_BADGES: LazyLock<HashMap<String, Vec<BadgesCache>>> = LazyLock::new(|| {
        serde_json::from_str::<HashMap<String, Vec<BadgesCache>>>(res_str!("badges.json")).unwrap()
    });

    pub async fn load_badge(set: String, id: String) -> bool {
        static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

        let mut loaded = false;

        BADGE_CACHE
            .get_with((set.clone(), id.clone()), || {
                Arc::new(tokio::sync::OnceCell::new())
            })
            .get_or_init(async || {
                let url: Cow<'static, str> = SAVED_BADGES
                    .get(&set)
                    .ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, ""))?
                    .iter()
                    .find(|s| s.id == id)
                    .ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, ""))?
                    .image
                    .clone();

                let data = CLIENT
                    .get(format!("{url}1"))
                    .header("Accept", "image/webp,image/png,image/gif,image/avif")
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?;

                loaded = true;

                Ok(Handle::from_bytes(data))
            })
            .await;

        loaded
    }
}

pub mod emotes {
    use std::{
        sync::{Arc, LazyLock},
        time::Duration,
    };

    use moka::policy::EvictionPolicy;
    use tokio::sync::OnceCell;

    use crate::widget::animated::AnimatedImage;

    type EmoteCache = moka::sync::Cache<String, Arc<OnceCell<anyhow::Result<AnimatedImage>>>>;

    pub static EMOTE_CACHE: LazyLock<EmoteCache> = LazyLock::new(|| {
        moka::sync::CacheBuilder::new(300)
            .eviction_policy(EvictionPolicy::tiny_lfu())
            .time_to_idle(Duration::from_secs(60 * 30))
            .name("badges")
            .build()
    });

    pub async fn load_emote(id: String) -> bool {
        static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

        let mut loaded = false;

        EMOTE_CACHE
            .get_with_by_ref(&id, || Arc::new(tokio::sync::OnceCell::new()))
            .get_or_init(async || {
                let data = CLIENT
                    .get(format!(
                        "https://static-cdn.jtvnw.net/emoticons/v2/{id}/default/dark/1.0"
                    ))
                    .header("Accept", "image/webp,image/png,image/gif,image/avif")
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?;

                let img = AnimatedImage::from_bytes(&data)?;

                loaded = true;

                Ok(img)
            })
            .await;

        loaded
    }
}
