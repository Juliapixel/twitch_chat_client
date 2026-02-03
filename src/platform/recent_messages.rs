use std::{fmt, sync::LazyLock};

use serde::{Deserialize, Deserializer, de::Visitor};
use twixel_core::IrcMessage;

use crate::util::default_client;

static RECENT_MESSAGES_API: LazyLock<url::Url> = LazyLock::new(|| {
    // compat wowie!
    std::env::var("CHATTERINO2_RECENT_MESSAGES_URL")
        .or_else(|_| std::env::var("RECENT_MESSAGES_URL"))
        .as_deref()
        .map(|e| e.parse().unwrap())
        .unwrap_or_else(|_| {
            "https://recent-messages.robotty.de/api/v2/"
                .parse()
                .unwrap()
        })
});

#[derive(Deserialize, Default)]
struct Response {
    _error: Option<String>,
    _error_code: Option<String>,
    #[serde(deserialize_with = "deser_irc")]
    messages: Vec<IrcMessage>,
}

fn deser_irc<'de, D>(deserializer: D) -> Result<Vec<IrcMessage>, D::Error>
where
    D: Deserializer<'de>,
{
    struct IrcVisitor(Vec<IrcMessage>);

    impl<'de> Visitor<'de> for IrcVisitor {
        type Value = Vec<IrcMessage>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an array of IRCv3 messages")
        }

        fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            while let Some(s) = seq.next_element::<String>()? {
                self.0
                    .push(IrcMessage::new(s).map_err(serde::de::Error::custom)?)
            }
            Ok(self.0)
        }
    }

    deserializer.deserialize_seq(IrcVisitor(Vec::new()))
}

pub async fn get_recent_messages(channel_login: &str) -> Vec<IrcMessage> {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(default_client);

    let mut url = RECENT_MESSAGES_API
        .join(&format!("recent-messages/{channel_login}"))
        .unwrap();
    url.set_query(Some("limit=250"));
    match CLIENT.get(url).send().await {
        Ok(r) => r.json::<Response>().await.unwrap_or_default().messages,
        Err(_) => Vec::new(),
    }
}
