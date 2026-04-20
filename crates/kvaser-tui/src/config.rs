use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use std::collections::BTreeMap;

use kvaser_core::DbcBinding;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct AppConfig {
    pub dbc_by_channel: BTreeMap<u32, Vec<PathBuf>>,
    pub dbc_files: Vec<PathBuf>,
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        parse_config(&text).unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let file = AppConfigFile::from_runtime(self);
        let text = toml::to_string_pretty(&file)?;
        fs::write(path, text)?;
        Ok(())
    }

    pub fn add_dbc_for_channel(&mut self, channel: u32, file: PathBuf) {
        let entry = self.dbc_by_channel.entry(channel).or_default();
        if !entry.iter().any(|existing| existing == &file) {
            entry.push(file);
        }
    }

    pub fn remove_dbc(&mut self, file: &Path) {
        for files in self.dbc_by_channel.values_mut() {
            files.retain(|entry| entry != file);
        }
        self.dbc_files.retain(|entry| entry != file);
    }

    pub fn clear_channel(&mut self, channel: u32) {
        self.dbc_by_channel.remove(&channel);
    }

    pub fn bindings(&self) -> Vec<DbcBinding> {
        let mut bindings = Vec::new();
        for (channel, files) in &self.dbc_by_channel {
            let mask = 1_u32.checked_shl(*channel).unwrap_or(0);
            if mask == 0 {
                continue;
            }
            for file in files {
                bindings.push(DbcBinding {
                    path: file.clone(),
                    channel_mask: mask,
                });
            }
        }

        if bindings.is_empty() {
            for file in &self.dbc_files {
                bindings.push(DbcBinding {
                    path: file.clone(),
                    channel_mask: 0x1f,
                });
            }
        }
        bindings
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct AppConfigFile {
    #[serde(default)]
    dbc_by_channel: Vec<ChannelDbcFiles>,
    #[serde(default)]
    dbc_files: Vec<PathBuf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ChannelDbcFiles {
    channel: u32,
    #[serde(default)]
    files: Vec<PathBuf>,
}

impl AppConfigFile {
    fn into_runtime(self) -> AppConfig {
        let mut map = BTreeMap::new();
        for entry in self.dbc_by_channel {
            if !entry.files.is_empty() {
                map.insert(entry.channel, entry.files);
            }
        }
        AppConfig {
            dbc_by_channel: map,
            dbc_files: self.dbc_files,
        }
    }

    fn from_runtime(config: &AppConfig) -> Self {
        let dbc_by_channel = config
            .dbc_by_channel
            .iter()
            .map(|(channel, files)| ChannelDbcFiles {
                channel: *channel,
                files: files.clone(),
            })
            .collect();
        Self {
            dbc_by_channel,
            dbc_files: config.dbc_files.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct LegacyAppConfigFile {
    #[serde(default)]
    dbc_by_channel: BTreeMap<String, Vec<PathBuf>>,
    #[serde(default)]
    dbc_files: Vec<PathBuf>,
}

fn parse_config(text: &str) -> Option<AppConfig> {
    if let Ok(file) = toml::from_str::<AppConfigFile>(text) {
        return Some(file.into_runtime());
    }

    let legacy = toml::from_str::<LegacyAppConfigFile>(text).ok()?;
    let mut map = BTreeMap::new();
    for (key, files) in legacy.dbc_by_channel {
        if let Ok(channel) = key.parse::<u32>() {
            map.insert(channel, files);
        }
    }
    Some(AppConfig {
        dbc_by_channel: map,
        dbc_files: legacy.dbc_files,
    })
}

pub fn config_path() -> PathBuf {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/kvaser_tui/config.toml")
}
