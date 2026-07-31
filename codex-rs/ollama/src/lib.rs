mod client;
mod line_buffer;
mod parser;
mod pull;
mod url;

pub use client::OllamaClient;
pub use client::OllamaModelMetadata;
use codex_core::config::Config;
use codex_model_provider_info::ModelProviderInfo;
pub use pull::CliProgressReporter;
pub use pull::PullEvent;
pub use pull::PullProgressReporter;
pub use pull::TuiProgressReporter;
use semver::Version;

/// Default OSS model to use when `--oss` is passed without an explicit `-m`.
pub const DEFAULT_OSS_MODEL: &str = "gpt-oss:20b";

/// Prepare the configured OSS environment when `--oss` is selected.
///
/// - Ensures the configured Ollama server is reachable.
/// - Checks if the model exists on that server and pulls it if missing.
pub async fn ensure_oss_ready(config: &Config) -> std::io::Result<()> {
    // Only download when the requested model is the default OSS model (or when -m is not provided).
    let model = match config.model.as_ref() {
        Some(model) => model,
        None => DEFAULT_OSS_MODEL,
    };

    // Verify the configured Ollama server is reachable.
    let ollama_client = crate::OllamaClient::try_from_oss_provider(config).await?;

    // If the model is not present on the configured server, pull it there.
    match ollama_client.fetch_models().await {
        Ok(models) => {
            if !models.iter().any(|m| m == model) {
                let mut reporter = crate::CliProgressReporter::new();
                ollama_client
                    .pull_with_reporter(model, &mut reporter)
                    .await?;
            }
        }
        Err(err) => {
            // Not fatal; higher layers may still proceed and surface errors later.
            tracing::warn!("Failed to query models from Ollama: {}.", err);
        }
    }

    Ok(())
}

fn min_responses_version() -> Version {
    Version::new(0, 13, 4)
}

fn supports_responses(version: &Version) -> bool {
    *version == Version::new(0, 0, 0) || *version >= min_responses_version()
}

/// Ensure the running Ollama server is new enough to support the Responses API.
///
/// Returns `Ok(())` when the version endpoint is missing or unparsable.
pub async fn ensure_responses_supported(provider: &ModelProviderInfo) -> std::io::Result<()> {
    let client = crate::OllamaClient::try_from_provider(provider).await?;
    ensure_client_responses_supported(&client).await
}

/// Ensure a named Ollama source is new enough to support the Responses API.
pub async fn ensure_responses_supported_at(base_url: &str) -> std::io::Result<()> {
    let client = crate::OllamaClient::try_from_base_url(base_url).await?;
    ensure_client_responses_supported(&client).await
}

async fn ensure_client_responses_supported(client: &OllamaClient) -> std::io::Result<()> {
    let Some(version) = client.fetch_version().await? else {
        return Ok(());
    };

    if supports_responses(&version) {
        return Ok(());
    }

    let min = min_responses_version();
    Err(std::io::Error::other(format!(
        "Ollama {version} is too old. The Responses API requires Ollama {min} or newer."
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_responses_for_dev_zero() {
        assert!(supports_responses(&Version::new(0, 0, 0)));
    }

    #[test]
    fn does_not_support_responses_before_cutoff() {
        assert!(!supports_responses(&Version::new(0, 13, 3)));
    }

    #[test]
    fn supports_responses_at_or_after_cutoff() {
        assert!(supports_responses(&Version::new(0, 13, 4)));
        assert!(supports_responses(&Version::new(0, 14, 0)));
    }
}
