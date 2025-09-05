use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

use crate::{config::Config, request::parse_response};

#[derive(Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginResponse {
    token: String,
}

pub async fn login(conf: &Config) -> anyhow::Result<()> {
    print!("Enter team name: ");
    io::stdout().flush().unwrap();

    let mut team_name = String::new();
    io::stdin().read_line(&mut team_name)
        .with_context(|| "failed to read team name")?;

    let team_name = team_name.trim().to_string();
    if team_name.is_empty() {
        bail!("team name cannot be empty");
    }

    let password = rpassword::prompt_password("Enter password: ")
        .with_context(|| "failed to read password")?;

    if password.is_empty() {
        bail!("password cannot be empty");
    }

    let response = reqwest::Client::new()
        .post(format!("{}/auth/login", conf.api_url))
        .json(&LoginRequest {
            username: team_name.clone(),
            password
        })
        .send()
        .await?;

    let login_response = parse_response::<LoginResponse>(response).await?;
    std::fs::write(crate::JWT_NAME, &login_response.token)
        .context("Failed to save auth token")?;
    
    println!("login successful for team: {}", team_name);
    
    Ok(())
}
