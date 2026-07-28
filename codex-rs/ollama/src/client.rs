use futures::StreamExt;
use futures::stream::BoxStream;
use semver::Version;
use serde_json::Value as JsonValue;
use std::collections::VecDeque;
use std::io;

use crate::line_buffer::LineBuffer;
use crate::parser::pull_events_from_value;
use crate::pull::PullEvent;
use crate::pull::PullProgressReporter;
use crate::url::base_url_to_host_root;
use crate::url::is_openai_compatible_base_url;
use codex_core::config::Config;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
#[cfg(test)]
use codex_model_provider_info::WireApi;
#[cfg(test)]
use codex_model_provider_info::create_oss_provider_with_base_url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaModelMetadata {
    pub capabilities: Vec<String>,
    pub context_window: Option<i64>,
}

/// Client for interacting with a configured Ollama instance.
pub struct OllamaClient {
    client: reqwest::Client,
    host_root: String,
    uses_openai_compat: bool,
}

impl OllamaClient {
    /// Construct a client for the built‑in open‑source ("oss") model provider
    /// and verify that the configured Ollama server is reachable. If no server is
    /// detected, returns an error with helpful installation/run instructions.
    pub async fn try_from_oss_provider(config: &Config) -> io::Result<Self> {
        // Note that we must look up the provider from the Config to ensure that
        // any overrides the user has in their config.toml are taken into
        // account.
        let provider = config
            .model_providers
            .get(OLLAMA_OSS_PROVIDER_ID)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Built-in provider {OLLAMA_OSS_PROVIDER_ID} not found",),
                )
            })?;

        Self::try_from_provider(provider).await
    }

    #[cfg(test)]
    async fn try_from_provider_with_base_url(base_url: &str) -> io::Result<Self> {
        let provider = create_oss_provider_with_base_url(base_url, WireApi::Responses);
        Self::try_from_provider(&provider).await
    }

    /// Build a client from a provider definition and verify the server is reachable.
    pub(crate) async fn try_from_provider(provider: &ModelProviderInfo) -> io::Result<Self> {
        #![expect(clippy::expect_used)]
        let base_url = provider
            .base_url
            .as_ref()
            .expect("oss provider must have a base_url");
        let uses_openai_compat = is_openai_compatible_base_url(base_url);
        let host_root = base_url_to_host_root(base_url);
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let client = Self {
            client,
            host_root,
            uses_openai_compat,
        };
        client.probe_server().await?;
        Ok(client)
    }

    /// Probe whether the server is reachable by hitting the appropriate health endpoint.
    async fn probe_server(&self) -> io::Result<()> {
        let url = if self.uses_openai_compat {
            format!("{}/v1/models", self.host_root.trim_end_matches('/'))
        } else {
            format!("{}/api/tags", self.host_root.trim_end_matches('/'))
        };
        let resp = self.client.get(url).send().await.map_err(|err| {
            tracing::warn!("Failed to connect to Ollama server: {err:?}");
            self.connection_error()
        })?;
        if resp.status().is_success() {
            Ok(())
        } else {
            tracing::warn!(
                "Failed to probe server at {}: HTTP {}",
                self.host_root,
                resp.status()
            );
            Err(self.connection_error())
        }
    }

    fn connection_error(&self) -> io::Error {
        io::Error::other(format!(
            "Cannot reach Ollama at {}. Check that the server is running and that its bind address and firewall allow this connection.",
            self.host_root
        ))
    }

    /// Return the list of model names known to the configured Ollama instance.
    pub async fn fetch_models(&self) -> io::Result<Vec<String>> {
        let tags_url = format!("{}/api/tags", self.host_root.trim_end_matches('/'));
        let resp = self
            .client
            .get(tags_url)
            .send()
            .await
            .map_err(io::Error::other)?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }
        let val = resp.json::<JsonValue>().await.map_err(io::Error::other)?;
        let names = val
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(names)
    }

    /// Return the capabilities advertised by Ollama for a model.
    pub async fn fetch_model_capabilities(&self, model: &str) -> io::Result<Vec<String>> {
        Ok(self.fetch_model_metadata(model).await?.capabilities)
    }

    /// Return the model metadata advertised by Ollama.
    pub async fn fetch_model_metadata(&self, model: &str) -> io::Result<OllamaModelMetadata> {
        let show_url = format!("{}/api/show", self.host_root.trim_end_matches('/'));
        let resp = self
            .client
            .post(show_url)
            .json(&serde_json::json!({"model": model}))
            .send()
            .await
            .map_err(io::Error::other)?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "failed to query Ollama capabilities for {model}: HTTP {}",
                resp.status()
            )));
        }
        let val = resp.json::<JsonValue>().await.map_err(io::Error::other)?;
        let capabilities = val
            .get("capabilities")
            .and_then(JsonValue::as_array)
            .map(|capabilities| {
                capabilities
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let context_window = val
            .get("model_info")
            .and_then(JsonValue::as_object)
            .and_then(|model_info| {
                model_info.iter().find_map(|(key, value)| {
                    key.ends_with(".context_length")
                        .then(|| value.as_i64())
                        .flatten()
                        .filter(|context_window| *context_window > 0)
                })
            });
        Ok(OllamaModelMetadata {
            capabilities,
            context_window,
        })
    }

    /// Query the server for its version string, returning `None` when unavailable.
    pub async fn fetch_version(&self) -> io::Result<Option<Version>> {
        let version_url = format!("{}/api/version", self.host_root.trim_end_matches('/'));
        let resp = self
            .client
            .get(version_url)
            .send()
            .await
            .map_err(io::Error::other)?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let val = resp.json::<JsonValue>().await.map_err(io::Error::other)?;
        let Some(version_str) = val.get("version").and_then(|v| v.as_str()).map(str::trim) else {
            return Ok(None);
        };
        let normalized = version_str.trim_start_matches('v');
        match Version::parse(normalized) {
            Ok(version) => Ok(Some(version)),
            Err(err) => {
                tracing::warn!("Failed to parse Ollama version `{version_str}`: {err}");
                Ok(None)
            }
        }
    }

    /// Start a model pull and emit streaming events. The returned stream ends when
    /// a Success event is observed or the server closes the connection.
    pub async fn pull_model_stream(
        &self,
        model: &str,
    ) -> io::Result<BoxStream<'static, PullEvent>> {
        let url = format!("{}/api/pull", self.host_root.trim_end_matches('/'));
        let resp = self
            .client
            .post(url)
            .json(&serde_json::json!({"model": model, "stream": true}))
            .send()
            .await
            .map_err(io::Error::other)?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "failed to start pull: HTTP {}",
                resp.status()
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut buf = LineBuffer::default();
        let _pending: VecDeque<PullEvent> = VecDeque::new();

        // Using an async stream adaptor backed by unfold-like manual loop.
        let s = async_stream::stream! {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buf.extend_from_slice(&bytes);
                        while let Some(line) = buf.take_line() {
                            if let Ok(text) = std::str::from_utf8(&line) {
                                let text = text.trim();
                                if text.is_empty() { continue; }
                                if let Ok(value) = serde_json::from_str::<JsonValue>(text) {
                                    for ev in pull_events_from_value(&value) { yield ev; }
                                    if let Some(err_msg) = value.get("error").and_then(|e| e.as_str()) {
                                        yield PullEvent::Error(err_msg.to_string());
                                        return;
                                    }
                                    if let Some(status) = value.get("status").and_then(|s| s.as_str())
                                        && status == "success" { yield PullEvent::Success; return; }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Connection error: end the stream.
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(s))
    }

    /// High-level helper to pull a model and drive a progress reporter.
    pub async fn pull_with_reporter(
        &self,
        model: &str,
        reporter: &mut dyn PullProgressReporter,
    ) -> io::Result<()> {
        reporter.on_event(&PullEvent::Status(format!("Pulling model {model}...")))?;
        let mut stream = self.pull_model_stream(model).await?;
        while let Some(event) = stream.next().await {
            reporter.on_event(&event)?;
            match event {
                PullEvent::Success => {
                    return Ok(());
                }
                PullEvent::Error(err) => {
                    // Empirically, ollama returns a 200 OK response even when
                    // the output stream includes an error message. Verify with:
                    //
                    // `curl -i http://localhost:11434/api/pull -d '{ "model": "foobarbaz" }'`
                    //
                    // As such, we have to check the event stream, not the
                    // HTTP response status, to determine whether to return Err.
                    return Err(io::Error::other(format!("Pull failed: {err}")));
                }
                PullEvent::ChunkProgress { .. } | PullEvent::Status(_) => {
                    continue;
                }
            }
        }
        Err(io::Error::other(
            "Pull stream ended unexpectedly without success.",
        ))
    }

    /// Low-level constructor given a raw host root, e.g. "http://localhost:11434".
    #[cfg(test)]
    fn from_host_root(host_root: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            host_root: host_root.into(),
            uses_openai_compat: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use pretty_assertions::assert_eq;

    // Happy-path tests using a mock HTTP server; skip if sandbox network is disabled.
    #[tokio::test]
    async fn test_fetch_models_happy_path() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_fetch_models_happy_path",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw(
                    serde_json::json!({
                        "models": [ {"name": "llama3.2:3b"}, {"name":"mistral"} ]
                    })
                    .to_string(),
                    "application/json",
                ),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::from_host_root(server.uri());
        let models = client.fetch_models().await.expect("fetch models");
        assert!(models.contains(&"llama3.2:3b".to_string()));
        assert!(models.contains(&"mistral".to_string()));
    }

    #[tokio::test]
    async fn test_fetch_model_metadata() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_fetch_model_metadata",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/show"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "capabilities": ["completion", "tools", "thinking"],
                    "model_info": {
                        "gptoss.context_length": 131_072,
                    },
                })),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::from_host_root(server.uri());
        let actual = client
            .fetch_model_metadata("gpt-oss:20b")
            .await
            .expect("model metadata");

        assert_eq!(
            actual,
            OllamaModelMetadata {
                capabilities: vec![
                    "completion".to_string(),
                    "tools".to_string(),
                    "thinking".to_string(),
                ],
                context_window: Some(131_072),
            }
        );
    }

    #[tokio::test]
    async fn test_fetch_version() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_fetch_version",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_raw(
                serde_json::json!({ "models": [] }).to_string(),
                "application/json",
            ))
            .mount(&server)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/version"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_raw(
                serde_json::json!({ "version": "0.14.1" }).to_string(),
                "application/json",
            ))
            .mount(&server)
            .await;

        let client = OllamaClient::try_from_provider_with_base_url(server.uri().as_str())
            .await
            .expect("client");

        let version = client.fetch_version().await.expect("version fetch");
        assert_eq!(version, Some(Version::new(0, 14, 1)));
    }

    #[tokio::test]
    async fn test_pull_model_stream_parses_large_json_lines() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_pull_model_stream_parses_large_json_lines",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        let body = format!(
            "{}\n{}\n",
            serde_json::json!({
                "status": "pulling layers",
                "padding": "x".repeat(128 * 1024),
            }),
            serde_json::json!({"status": "complete"}),
        );
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/pull"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw(body, "application/x-ndjson"),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::from_host_root(server.uri());
        let events = client
            .pull_model_stream("test-model")
            .await
            .expect("start pull stream")
            .collect::<Vec<_>>()
            .await;

        assert_matches!(
            events.as_slice(),
            [
                PullEvent::Status(pulling),
                PullEvent::Status(complete),
            ] if pulling == "pulling layers" && complete == "complete"
        );
    }

    #[tokio::test]
    async fn test_probe_server_happy_path_openai_compat_and_native() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_probe_server_happy_path_openai_compat_and_native",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;

        // Native endpoint
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let native = OllamaClient::from_host_root(server.uri());
        native.probe_server().await.expect("probe native");

        // OpenAI compatibility endpoint
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/models"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let ollama_client =
            OllamaClient::try_from_provider_with_base_url(&format!("{}/v1", server.uri()))
                .await
                .expect("probe OpenAI compat");
        ollama_client
            .probe_server()
            .await
            .expect("probe OpenAI compat");
    }

    #[tokio::test]
    async fn test_try_from_oss_provider_ok_when_server_running() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_try_from_oss_provider_ok_when_server_running",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;

        // OpenAI‑compat models endpoint responds OK.
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/models"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        OllamaClient::try_from_provider_with_base_url(&format!("{}/v1", server.uri()))
            .await
            .expect("client should be created when probe succeeds");
    }

    #[tokio::test]
    async fn test_try_from_oss_provider_err_when_server_missing() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_try_from_oss_provider_err_when_server_missing",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        let err = OllamaClient::try_from_provider_with_base_url(&format!("{}/v1", server.uri()))
            .await
            .err()
            .expect("expected error");
        assert_eq!(
            err.to_string(),
            format!(
                "Cannot reach Ollama at {}. Check that the server is running and that its bind address and firewall allow this connection.",
                server.uri()
            )
        );
    }
}
