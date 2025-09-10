use std::{path::Path, process::Command};
use anyhow::{bail, Context, Result};
use crate::config::Config;

const CLI_REPO_URL: &str = "https://github.com/mechmania/cli";

pub async fn check_all_updates(root: &Path, config: &Config) -> Result<()> {
    let (cli_updates, starterpack_updates) = tokio::join!(
        has_cli_updates(),
        async { has_upstream_changes(root, config) }
    );
    
    match (cli_updates?, starterpack_updates?) {
        (true, true) => println!("Updates available for CLI and starterpack! Run with 'update' command"),
        (true, false) => println!("CLI update available! Run with 'update' command"),
        (false, true) => println!("Starterpack update available! Run with 'update' command"),
        (false, false) => {} // Silent when up to date
    }
    Ok(())
}

pub async fn update_all(root: &Path, config: &Config) -> Result<()> {
    let (cli_needs_update, starterpack_needs_update) = tokio::join!(
        has_cli_updates(),
        async { has_upstream_changes(root, config) }
    );

    let (cli_needs_update, starterpack_needs_update) = (cli_needs_update?, starterpack_needs_update?);
    
    if cli_needs_update {
        update_cli().await?;
    }
    
    if starterpack_needs_update {
        update_starterpack(root, config)?;
    }
    
    if !cli_needs_update && !starterpack_needs_update {
        println!("Everything is up to date!");
    }
    
    Ok(())
}

async fn has_cli_updates() -> Result<bool> {
    let current_hash = get_current_cli_hash();
    let latest_hash = get_remote_cli_hash().await?;
    
    Ok(current_hash != latest_hash)
}

fn get_current_cli_hash() -> &'static str {
    env!("GIT_HASH")
}

async fn get_remote_cli_hash() -> Result<String> {
    let output = Command::new("git")
        .args(["ls-remote", CLI_REPO_URL, "HEAD"])
        .output()
        .context("Failed to check remote CLI version")?;
    
    if !output.status.success() {
        bail!("Failed to fetch remote hash: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    let remote_info = String::from_utf8(output.stdout)?;
    let hash = remote_info
        .split_whitespace()
        .next()
        .context("Invalid remote response")?;
    
    Ok(hash.to_string())
}

async fn update_cli() -> Result<()> {
    println!("Updating CLI...");
    
    let output = Command::new("cargo")
        .args([
            "install", 
            "--git", 
            CLI_REPO_URL,
            "--force"
        ])
        .output()
        .context("Failed to update CLI")?;
    
    if !output.status.success() {
        bail!("CLI update failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    println!("CLI updated successfully");
    Ok(())
}

fn has_upstream_changes(root: &Path, config: &Config) -> Result<bool> {
    add_upstream_remote(root, config)?;
    
    let output = Command::new("git")
        .args(["fetch", "upstream", "main"])
        .current_dir(root)
        .output()
        .context("Failed to fetch upstream")?;
    
    if !output.status.success() {
        bail!("Git fetch failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD..upstream/main"])
        .current_dir(root)
        .output()
        .context("Failed to check for updates")?;
    
    if !output.status.success() {
        bail!("Git rev-list failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    let count = String::from_utf8(output.stdout)?
        .trim()
        .parse::<u32>()?;
    
    Ok(count > 0)
}

fn update_starterpack(root: &Path, config: &Config) -> Result<()> {
    println!("Updating starterpack...");
    
    preserve_strategy_files(root, config)?;
    
    let output = Command::new("git")
        .args(["merge", "upstream/main", "--no-edit"])
        .current_dir(root)
        .output()
        .context("Failed to merge updates")?;
        
    if !output.status.success() {
        bail!("Git merge failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    println!("Starterpack updated successfully");
    Ok(())
}

fn add_upstream_remote(root: &Path, config: &Config) -> Result<()> {
    let repo_url = get_starterpack_url(config);
    
    Command::new("git")
        .args(["remote", "add", "upstream", repo_url])
        .current_dir(root)
        .output()?;
        
    Ok(())
}

fn get_starterpack_url(config: &Config) -> &'static str {
    use crate::config::Lang;
    match config.language {
        Lang::Rust => "https://github.com/mechmania/rust-starterpack",
        Lang::Python => "https://github.com/mechmania/python-starterpack",
        Lang::Java => "https://github.com/mechmania/java-starterpack",
    }
}

fn preserve_strategy_files(root: &Path, config: &Config) -> Result<()> {
    use crate::config::Lang;
    let strategy_dir = match config.language {
        Lang::Rust => "src/strategy",
        Lang::Python => "strategy",
        Lang::Java => "src/com/bot/strategy",
    };
    
    Command::new("git")
        .args(["add", strategy_dir])
        .current_dir(root)
        .output()
        .context("Failed to preserve strategy files")?;
        
    Ok(())
}
