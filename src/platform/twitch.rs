pub mod badges {
    use std::{borrow::Cow, collections::HashMap, sync::LazyLock, time::Duration};

    use iced::widget::image::{Handle, Image};
    use moka::policy::EvictionPolicy;
    use serde::Deserialize;

    use crate::res_str;

    #[derive(Deserialize)]
    struct BadgesCache {
        #[serde(borrow)]
        id: Cow<'static, str>,
        #[serde(borrow)]
        image: Cow<'static, str>,
    }

    pub static BADGE_CACHE: LazyLock<
        moka::sync::Cache<(String, String), Handle>,
    > = LazyLock::new(|| {
        moka::sync::CacheBuilder::new(300)
            .eviction_policy(EvictionPolicy::tiny_lfu())
            .time_to_idle(Duration::from_secs(60 * 30))
            .name("badges")
            .build()
    });

    static SAVED_BADGES: LazyLock<HashMap<String, Vec<BadgesCache>>> = LazyLock::new(|| {
        serde_json::from_str::<HashMap<String, Vec<BadgesCache>>>(res_str!("badges.json")).unwrap()
    });

    pub async fn load_badge(set: String, id: String) -> Option<iced::widget::Image> {
        static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);
        let entry = BADGE_CACHE
            .get(&(set.clone(), id.clone()));
        let entry = match entry {
            Some(e) => {
                e
            },
            None => {
                let url: Cow<'static, str> = SAVED_BADGES
                    .get(&set)?
                    .iter()
                    .find(|s| s.id == id)?
                    .image
                    .clone();

                let data = CLIENT
                    .get(format!("{url}1"))
                    .header("Accept", "image/webp,image/png,image/gif,image/avif")
                    .send()
                    .await
                    .ok()?
                    .error_for_status()
                    .ok()?
                    .bytes()
                    .await
                    .ok()?;

                let handle = Handle::from_bytes(data);

                BADGE_CACHE.insert((set, id), handle.clone());
                handle
            }
        };

        Some(Image::new(entry))
    }
}

pub mod emotes {

}
