use std::{path::PathBuf, sync::LazyLock};

use clap::Parser;

#[derive(clap::Parser)]
pub struct Args {
    #[arg(long)]
    pub config: Option<PathBuf>,
}

pub static ARGS: LazyLock<Args> = LazyLock::new(Args::parse);
