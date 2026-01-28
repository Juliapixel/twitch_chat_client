use crate::widget::animated::{AnimatedImage, AnimatedImageError};

pub mod seventv;
pub mod twitch;

struct Emote {
    name: String,
    alias: Option<String>,
}

pub trait EmoteProvider {
    async fn get_channel_emotes(id: &str) -> Vec<Emote>;
    async fn get_emote(id: &str) -> Result<AnimatedImage, AnimatedImageError>;
}
