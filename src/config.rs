use std::{env, fmt::Display, fs, path::{Path, PathBuf}};

use anyhow::Context;
use serde::{ Serialize, Deserialize };

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub language: Lang,
    #[serde(rename = "api-url")]
    pub api_url: String,
}

#[derive(Serialize, Deserialize)]
pub enum Lang {
    #[serde(rename = "rust")]
    Rust,
    #[serde(rename = "python")]
    Python,
    #[serde(rename = "java")]
    Java
}

impl Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lang::Rust => write!(f, "rust"),
            Lang::Python => write!(f, "python"),
            Lang::Java => write!(f, "java"),
        }
    }
}

pub fn read(root: &Path) -> anyhow::Result<Config> {

    // println!("reading config file...");

    let file = root.join(crate::CONFIG_NAME);

    let content = fs::read_to_string(&file)
        .with_context(|| format!("Failed to read config file: {}", file.display()))?;

    let config: Config = toml::from_str(&content)
        .with_context(|| format!("failed to parse config from {}", file.display()))?;

    // println!("language is {}", config.language);
    // println!("url is {}", config.api_url);

    Ok(config)
}
