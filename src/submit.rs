use std::{io::{self, Write}, path::Path};
use crate::{
    config::Config, 
    request::{authenticate, parse_response}
};
use colored::Colorize;

use flate2::{Compression, write::GzEncoder};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tar::Builder;
use anyhow::{bail, Context, Result};
use base64::{Engine as _, engine::general_purpose};


#[derive(Serialize)]
struct SubmitRequest {
    language: String,
    data: String, 
}

#[derive(Deserialize)]
struct SubmitResponse {
    submission_id: u32,
}

#[derive(Deserialize)]
struct CompilationResponse {
    status: CompilationStatus,
    result: CompilationResult,
}

#[derive(Deserialize, Debug)]
enum CompilationStatus {
    #[serde(rename = "pending")]
    Pending = 0,
    #[serde(rename = "success")]
    Success = 1,
    #[serde(rename = "failure")]
    Failure = 2,
}

#[derive(Deserialize)]
struct CompilationResult {
    success: bool,
    error_message: Option<String>,
    build_log: String
}


pub fn compress_folder(folder_path: impl AsRef<Path>) -> Result<Box<[u8]>> {
    let folder_path = folder_path.as_ref();

    let buffer = Vec::new();

    let enc = GzEncoder::new(buffer, Compression::default());

    let mut tar = Builder::new(enc);

    tar.append_dir_all("strategy", folder_path)
        .context("failed to compress directory")?;

    tar.finish()
        .context("failed to finalize archive")?;
    
    let enc = tar.into_inner()
        .context("failed to get encoder")?;
    
    let compressed_data = enc.finish()
        .context("failed to finish compression")?;
    
    Ok(compressed_data.into_boxed_slice())
}

pub async fn submit(root: &Path, config: &Config) -> anyhow::Result<()> {
    use crate::config::Lang;
    let strategy_path = match config.language {
        Lang::Rust => "src/strategy",
        Lang::Python => "strategy",
        Lang::Java => "src/com/bot/strategy",
    };

    let strategy_path = root.join(strategy_path);
    if !strategy_path.exists() {
        bail!("could not find strategy code: {} does not exist", strategy_path.display())
    }

    let data = compress_folder(strategy_path)?;

    let encoded_data = general_purpose::STANDARD.encode(&*data);
    
    let client = Client::new();
    
    println!("submitting bot...");
    let submit_request = SubmitRequest {
        language: format!("{}", config.language),
        data: encoded_data,
    };
    
    let response = authenticate(root, client.post(format!("{}/bot/submit", config.api_url)))?
        .json(&submit_request)
        .send()
        .await
        .context("failed to submit bot")?;
    
    let submit_response: SubmitResponse = parse_response(response).await?;
    let submission_id = &submit_response.submission_id;
    
    println!("{}", "uploaded successfully and queued for submission".green());
    

    let compilation: Option<CompilationResponse>;

    // poll
    print!("polling submission status (canceling here will not abort the submission)");
    io::stdout().flush().unwrap();
    loop {
        print!(".");
        io::stdout().flush().unwrap();
        let response = authenticate(root, client.get(&format!("{}/bot/compilation/{}", config.api_url, submission_id)))?
            .send()
            .await
            .context("failed to check submission status")?;
        
        let status_response: CompilationResponse = parse_response(response).await?;
        
        if !matches!(status_response.status, CompilationStatus::Pending) {
            compilation = Some(status_response);
            break;
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    println!();

    let compilation = compilation.unwrap();

    if !compilation.result.success {
        println!("{}", "submission failed".red());
        if let Some(reason) = compilation.result.error_message {
            println!("reason: {}", reason);
        }
        println!("build log: \n\n{}", compilation.result.build_log);
        println!("for help, please reach out to us on discord");
        return Ok(());
    }

    println!("{}", "submission success".green());

    Ok(())
}
