use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::cli::ARGS;

pub static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| {
    log::info!(
        "Reading config from {}",
        &CONFIG_FILE_PATH.as_os_str().to_string_lossy()
    );
    let config = match Config::read_from_file(&CONFIG_FILE_PATH) {
        Ok(ok) => ok,
        Err(e) => {
            log::error!("{e}");
            std::process::exit(1)
        }
    };

    RwLock::new(config)
});

static CONFIG_FILE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    ARGS.config
        .clone()
        .unwrap_or_else(|| config_dir().join("config.toml"))
});

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub accounts: Vec<Account>,
    pub chats: Vec<String>,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Serialize, Deserialize)]
pub struct Account {
    username: String,
    token: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct UiConfig {
    #[serde(default)]
    pub natural_scrolling: bool,
}

impl Config {
    pub fn save(&mut self) -> Result<(), std::io::Error> {
        self.save_to_file(&CONFIG_FILE_PATH)
    }

    fn read_from_file(path: &Path) -> Result<Self, std::io::Error> {
        let res = std::fs::read_to_string(path)
            .and_then(|s| toml::from_str(&s).map_err(std::io::Error::other));
        if let Err(e) = &res
            && e.kind() == io::ErrorKind::NotFound
        {
            let mut new = Config::default();
            new.save_to_file(path)?;
            Ok(new)
        } else {
            res
        }
    }

    fn save_to_file(&mut self, path: &Path) -> Result<(), std::io::Error> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(path.parent().unwrap())?;
        let mut file = std::fs::OpenOptions::new()
            .truncate(true)
            .write(true)
            .create(true)
            .open(path)?;
        let toml = toml::ser::to_string_pretty(self).map_err(std::io::Error::other)?;
        #[cfg(unix)]
        {
            use std::{
                fs::Permissions,
                os::unix::fs::{MetadataExt, PermissionsExt},
            };

            // Safety: geteuid never fails.
            let uid = unsafe { libc::geteuid() };
            // Safety: getgid never fails.
            let gid = unsafe { libc::getgid() };

            if file.metadata()?.uid() != uid && file.metadata()?.gid() != gid {
                std::os::unix::fs::fchown(&file, Some(uid), Some(gid))?;
            }
            // -rw-------
            file.set_permissions(Permissions::from_mode(0o600))?;
        }
        file.write_all(toml.as_bytes())
    }
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("juliarino"))
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .map(|d| d.parent().unwrap().to_owned())
                .take_if(|p| !(p.exists() && p.is_dir()))
        })
        .unwrap_or_else(|| PathBuf::from("./"))
}
