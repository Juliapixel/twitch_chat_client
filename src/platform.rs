pub mod seventv;
pub mod twitch;

pub static DECODER_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
