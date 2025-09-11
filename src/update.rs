use std::{path::Path};
use tokio::process::Command;
use anyhow::{bail, Context, Result};
use crate::config::Config;

const CLI_REPO_URL: &str = "https://github.com/mechmania/cli";

pub async fn check_all_updates(root: &Path, config: &Config) -> Result<bool> {
    let (cli_updates, starterpack_updates) = tokio::join!(
        has_cli_updates(),
        has_upstream_changes(root, config)
    );
    Ok(cli_updates? || starterpack_updates?)
}

pub async fn update_all(root: &Path, config: &Config) -> Result<()> {
    let (cli_needs_update, starterpack_needs_update) = tokio::join!(
        has_cli_updates(),
        has_upstream_changes(root, config)
    );

    let (cli_needs_update, starterpack_needs_update) = (cli_needs_update?, starterpack_needs_update?);
    
    if cli_needs_update {
        update_cli().await?;
    }
    
    if starterpack_needs_update {
        update_starterpack(root, config).await?;
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
        .await
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
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()
        .await
        .context("Failed to update CLI")?;
    
    if !output.status.success() {
        bail!("CLI update failed");
    }
    
    println!("CLI updated successfully");
    Ok(())
}

async fn has_upstream_changes(root: &Path, config: &Config) -> Result<bool> {
    add_upstream_remote(root, config).await?;
    
    let output = Command::new("git")
        .args(["fetch", "upstream", "main"])
        .current_dir(root)
        .output()
        .await
        .context("Failed to fetch upstream")?;
    
    if !output.status.success() {
        bail!("Git fetch failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD..upstream/main"])
        .current_dir(root)
        .output()
        .await
        .context("Failed to check for updates")?;
    
    if !output.status.success() {
        bail!("Git rev-list failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    let count = String::from_utf8(output.stdout)?
        .trim()
        .parse::<u32>()?;
    
    Ok(count > 0)
}

async fn update_starterpack(root: &Path, config: &Config) -> Result<()> {
    println!("Updating starterpack...");
    
    let strategy_path = crate::strategy_path(config);
    let strategy_path_str = strategy_path.to_string_lossy();

    println!("restoring non-strategy files...");
    // restore from upstream, excluding strategy
    let output = Command::new("git")
        .args([
            "restore",
            "--source=upstream/main",
            "--",
            ".",
            &format!(":!{}", strategy_path_str),
            &format!(":!{}/**", strategy_path_str),
        ])
        .current_dir(root)
        .output()
        .await
        .context("Failed to run git restore")?;

    if !output.status.success() {
        bail!("Git restore failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    println!("stashing uncommitted changes in your code...");
    // stash, this will stash strategy changes
    
    let output = Command::new("git")
        .args([
            "stash",
        ])
        .current_dir(root)
        .output()
        .await
        .context("Failed to run git stash")?;

    if !output.status.success() {
        bail!("Git stash failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    println!("applying upstream changes...");
    // rebase
    let output = Command::new("git")
        .args([
            "rebase",
            "upstream/main",
        ])
        .current_dir(root)
        .output()
        .await
        .context("Failed to run git rebase")?;

    if !output.status.success() {
        bail!("Git rebase failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    println!("restoring your uncommitted changes...");
    // stash pop
    let output = Command::new("git")
        .args([
            "stash",
            "pop",
        ])
        .current_dir(root)
        .output()
        .await
        .context("Failed to run git stash pop")?;

    if !output.status.success() {
        bail!("Git stash pop failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    println!("Starterpack updated successfully");
    Ok(())
}

async fn add_upstream_remote(root: &Path, config: &Config) -> Result<()> {
    let repo_url = get_starterpack_url(config);
    
    Command::new("git")
        .args(["remote", "add", "upstream", repo_url])
        .current_dir(root)
        .output().await?;
        
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
