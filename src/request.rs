use std::{fs, path::Path};
use anyhow::Context;
use reqwest::{RequestBuilder, Response};
use serde::{de::DeserializeOwned, Deserialize };


#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
    details: Option<String>,
}

pub fn authenticate(root: &Path, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
    let file = root.join(crate::JWT_NAME);

    let content = fs::read_to_string(&file)
        .with_context(|| format!("failed to read certificate in {}\n\n have you logged in?", file.display()))?;

    let token = content.trim();
    Ok(req.bearer_auth(token))
}

pub async fn parse_response<T: DeserializeOwned>(response: Response) -> anyhow::Result<T> {
    match response.status() {
        status if status.is_success() => {
            response
                .json::<T>()
                .await
                .with_context(|| "Failed to parse response")
        }
        status => {
            if let Ok(error_response) = response.json::<ErrorResponse>().await {
                match status.as_u16() {
                    400 => anyhow::bail!("bad request: {}", error_response.error),
                    401 => anyhow::bail!("authentication failed: {}", error_response.error),
                    500 => {
                        let details = error_response.details
                            .map(|d| format!(" ({})", d))
                            .unwrap_or_default();
                        anyhow::bail!("server error: {}{}", error_response.error, details);
                    }
                    _ => anyhow::bail!("request failed ({}): {}", status, error_response.error),
                }
            } else {
                anyhow::bail!("request failed with status: {}", status);
            }
        }
    }
}
