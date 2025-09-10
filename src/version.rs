use std::path::Path;
use std::io::{self, Write};
use crate::{
    config::Config, 
    request::{authenticate, parse_response}
};
use anyhow::Context;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tabled::Tabled;

#[derive(Clone, Debug)]
pub enum Version {
    Number(u32),
    Latest,
}

pub fn parse_version(s: &str) -> Result<Version, String> {
    if s == "latest" {
        Ok(Version::Latest)
    } else {
        s.parse::<u32>()
            .map(Version::Number)
            .map_err(|_| format!("Invalid version: '{}'. Expected a number or 'latest'", s))
    }
}


#[derive(Deserialize)]
enum CompileStatus {
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "failure")]
    Failure,
}

impl std::fmt::Display for CompileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            CompileStatus::Success => "success",
            CompileStatus::Failure => "failure",
        })
    }
}

#[derive(Deserialize, Tabled)]
struct VersionResponse {
    #[serde(rename = "version")]
    version_number: u32,
    language: String,
    compile_status: CompileStatus,
    compiled_at: String,
    submitted_at: String,
}

#[derive(Deserialize)]
struct VersionsResponse {
    versions: Vec<VersionResponse>,
    active_version: Option<u32>,
}

impl std::fmt::Display for VersionsResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", tabled::Table::new(&self.versions))?;
        match self.active_version {
            Some(v) => writeln!(f, "Active version is {}", v),
            None => writeln!(f, "No active version set"),
        }
    }
}

#[derive(Serialize)]
struct SwitchRequest {
    version: u32
}

async fn get_versions(root: &Path, config: &Config) -> anyhow::Result<VersionsResponse> {
    let client = Client::new();
    // fetch current versions
    let versions = authenticate(root, client.get(format!("{}/bot/versions", config.api_url)))?
        .send()
        .await
        .context("failed to fetch bot versions")?;
    Ok(parse_response::<VersionsResponse>(versions).await?)
}


pub async fn list(root: &Path, config: &Config) -> anyhow::Result<()> {
    let versions = get_versions(root, config).await?;

    // Print table
    println!("{}", versions);

    // Show what "latest" means
    if let Some(max_version) = versions.versions.iter().map(|v| v.version_number).max() {
        println!("'latest' resolves to version {}", max_version);
    }

    Ok(())
}

pub async fn switch(args: crate::Switch, root: &Path, config: &Config) -> anyhow::Result<()> {
    let versions = get_versions(root, config).await?;

    // Resolve requested version
    let version = match args.version {
        Some(Version::Number(v)) => v,
        Some(Version::Latest) => {
            versions
                .versions
                .iter()
                .map(|vr| vr.version_number)
                .max()
                .context("No versions available to switch to")?
        }
        None => {
            // Show options
            println!("{}", versions);
            print!("Enter version number to switch to: ");
            io::stdout().flush().ok();

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .context("Failed to read input")?;
            let input = input.trim();

            parse_version(input)
                .map_err(|e| anyhow::anyhow!(e))
                .and_then(|v| match v {
                    Version::Number(n) => Ok(n),
                    Version::Latest => versions
                        .versions
                        .iter()
                        .map(|vr| vr.version_number)
                        .max()
                        .context("No versions available to switch to"),
                })?
        }
    };

    // Validate existence
    let version_info = versions
        .versions
        .iter()
        .find(|vr| vr.version_number == version)
        .with_context(|| format!("Version {} not found", version))?;

    // Validate compile status
    if !matches!(version_info.compile_status, CompileStatus::Success) {
        anyhow::bail!(
            "Version {} has status '{}', cannot switch",
            version,
            version_info.compile_status
        );
    }

    // Send request
    let client = Client::new();
    let resp = authenticate(
        root,
        client
            .post(format!("{}/bot/change-version", config.api_url))
            .json(&SwitchRequest { version }),
    )?
    .send()
    .await
    .context("failed to send change-version request")?;

    let text = resp.text().await.context("failed to read response body")?;
    println!("Server response: {}", text);

    Ok(())
}

