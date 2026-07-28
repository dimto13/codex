//! OSS provider utilities shared between TUI and exec.

use codex_core::config::Config;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use codex_ollama::OllamaModelMetadata;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;

/// Ollama provider selected when Aren is launched without local-provider flags.
pub const AREN_DEFAULT_OSS_PROVIDER: &str = OLLAMA_OSS_PROVIDER_ID;

/// Ollama model selected when Aren is launched without a model flag.
pub const AREN_DEFAULT_OLLAMA_MODEL: &str = "gemma4:e4b";

/// Aren release version embedded by the release build.
pub const AREN_VERSION: &str = match option_env!("AREN_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// Returns the default model for a given OSS provider.
pub fn get_default_model_for_oss_provider(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        LMSTUDIO_OSS_PROVIDER_ID => Some(codex_lmstudio::DEFAULT_OSS_MODEL),
        OLLAMA_OSS_PROVIDER_ID => Some(codex_ollama::DEFAULT_OSS_MODEL),
        _ => None,
    }
}

/// Ensures the specified OSS provider is ready (models downloaded, service reachable).
pub async fn ensure_oss_provider_ready(
    provider_id: &str,
    config: &Config,
) -> Result<(), std::io::Error> {
    match provider_id {
        LMSTUDIO_OSS_PROVIDER_ID => {
            codex_lmstudio::ensure_oss_ready(config)
                .await
                .map_err(|e| std::io::Error::other(format!("OSS setup failed: {e}")))?;
        }
        OLLAMA_OSS_PROVIDER_ID => {
            codex_ollama::ensure_responses_supported(&config.model_provider).await?;
            codex_ollama::ensure_oss_ready(config)
                .await
                .map_err(|e| std::io::Error::other(format!("OSS setup failed: {e}")))?;
        }
        _ => {
            // Unknown provider, skip setup
        }
    }
    Ok(())
}

/// Installs an Aren-local model catalog backed by the models available in Ollama.
///
/// The static catalog prevents the embedded model manager from querying Ollama's
/// OpenAI-compatible `/v1/models` endpoint with the OpenAI catalog schema.
pub async fn configure_aren_oss_model_catalog(
    provider_id: &str,
    config: &mut Config,
) -> Result<(), std::io::Error> {
    if provider_id != OLLAMA_OSS_PROVIDER_ID {
        return Ok(());
    }

    let selected_model = config
        .model
        .clone()
        .unwrap_or_else(|| codex_ollama::DEFAULT_OSS_MODEL.to_string());
    let client = codex_ollama::OllamaClient::try_from_oss_provider(config).await?;
    let selected_metadata = client.fetch_model_metadata(&selected_model).await?;
    let supports_thinking = supports_thinking(&selected_metadata);
    config.model_reasoning_effort =
        normalize_ollama_reasoning_effort(config.model_reasoning_effort.take(), supports_thinking);

    let mut model_names = client.fetch_models().await?;
    model_names.sort();
    model_names.dedup();
    model_names.retain(|model| model != &selected_model);
    model_names.insert(0, selected_model.clone());

    let mut models = Vec::with_capacity(model_names.len());
    for (priority, model) in model_names.into_iter().enumerate() {
        let metadata = if model == selected_model {
            Some(selected_metadata.clone())
        } else {
            client.fetch_model_metadata(&model).await.ok()
        };
        models.push(aren_ollama_model_info(
            &model,
            metadata.as_ref(),
            i32::try_from(priority).unwrap_or(i32::MAX),
        ));
    }
    config.model_catalog = Some(ModelsResponse { models });
    Ok(())
}

fn aren_ollama_model_info(
    model: &str,
    metadata: Option<&OllamaModelMetadata>,
    priority: i32,
) -> ModelInfo {
    let supports_thinking = metadata.is_some_and(supports_thinking);
    let mut model_info = codex_models_manager::model_info::model_info_from_slug(model);
    model_info.description = Some("Installed locally in Ollama.".to_string());
    model_info.default_reasoning_level = Some(if supports_thinking {
        ReasoningEffort::High
    } else {
        ReasoningEffort::None
    });
    model_info.supported_reasoning_levels = ollama_reasoning_levels(supports_thinking);
    model_info.visibility = ModelVisibility::List;
    model_info.priority = priority;
    model_info.base_instructions = aren_base_instructions(&model_info.base_instructions);
    model_info.default_reasoning_summary = ReasoningSummary::None;
    model_info.used_fallback_model_metadata = false;
    if let Some(context_window) = metadata.and_then(|metadata| metadata.context_window) {
        model_info.context_window = Some(context_window);
        model_info.max_context_window = Some(context_window);
    }
    model_info
}

fn supports_thinking(metadata: &OllamaModelMetadata) -> bool {
    metadata
        .capabilities
        .iter()
        .any(|capability| capability == "thinking")
}

fn ollama_reasoning_levels(supports_thinking: bool) -> Vec<ReasoningEffortPreset> {
    if !supports_thinking {
        return vec![ReasoningEffortPreset {
            effort: ReasoningEffort::None,
            description: "This model does not advertise reasoning support".to_string(),
        }];
    }

    vec![
        ReasoningEffortPreset {
            effort: ReasoningEffort::None,
            description: "Disable extended reasoning".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::Low,
            description: "Fast responses with lighter reasoning".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "Balance speed and reasoning depth".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::High,
            description: "Greater reasoning depth for complex problems".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffort::Max,
            description: "Maximum reasoning depth".to_string(),
        },
    ]
}

fn normalize_ollama_reasoning_effort(
    effort: Option<ReasoningEffort>,
    supports_thinking: bool,
) -> Option<ReasoningEffort> {
    if !supports_thinking {
        return Some(ReasoningEffort::None);
    }

    match effort {
        Some(
            effort @ (ReasoningEffort::None
            | ReasoningEffort::Low
            | ReasoningEffort::Medium
            | ReasoningEffort::High
            | ReasoningEffort::Max),
        ) => Some(effort),
        Some(ReasoningEffort::Minimal) => Some(ReasoningEffort::Low),
        Some(ReasoningEffort::XHigh) => Some(ReasoningEffort::High),
        Some(ReasoningEffort::Ultra) => Some(ReasoningEffort::Max),
        Some(ReasoningEffort::Custom(_)) | None => Some(ReasoningEffort::High),
    }
}

fn aren_base_instructions(instructions: &str) -> String {
    let instructions = instructions
        .replace(
            "You are a coding agent running in the Codex CLI, a terminal-based coding assistant. Codex CLI is an open source project led by OpenAI.",
            "You are a coding agent running in the Aren CLI, a terminal-based local coding assistant. Aren is a local-first agentic coding interface.",
        )
        .replace(
            "Within this context, Codex refers to the open-source agentic coding interface (not the old Codex language model built by OpenAI).",
            "Within this context, Aren refers to the local agentic coding interface.",
        );

    format!(
        "{instructions}\n\n# Autonomous continuation\n- The permission state supplied by the runtime is authoritative. When the approval policy is `never` and the sandbox is disabled or has full access, continue through ordinary safe steps of the user's task without asking for textual confirmation such as `Soll ich fortfahren?`, `go?`, or `continue?`.\n- Do not split an already requested multi-step task into phase-by-phase approval checkpoints. A completed tool call or work phase is not a reason to stop; continue until the user's request is resolved.\n- Ask a question only when a missing user choice would materially change the result, required authorization or external coordination is unavailable, or a risky irreversible action falls outside the user's existing request.\n- Never invent an approval requirement after `/permissions` has granted the access needed for the next step.\n\n# Local workspace access\n- You have direct access to the current working directory, local files, the operating system, and local Git repositories through `exec_command` or `shell_command`.\n- For questions about the current project, files, directories, Git history or status, installed software, or other local state, inspect the real state with a suitable read-only command before answering.\n- Treat phrases such as `this project`, `here`, `dieses Projekt`, or `von hier` as referring to the current working directory unless the user names another location.\n- Prefer standard command-line programs such as `git` and `rg`; a new task-specific Python implementation is not needed for ordinary local inspection.\n- Never claim that local or Git access is unavailable while `exec_command` or `shell_command` is available.\n\n# Current date\n- The current local date is provided on every turn in the `<current_date>` element of the environment context, formatted as `YYYY-MM-DD`.\n- When the user asks for today's date, the current month, or the current year, answer directly from that system-provided date; never substitute a date from model memory and do not ask for clarification.\n- Call `mcp__executor__get_current_datetime` when the user asks for the current local time or a timezone-aware timestamp. Use its result exactly.\n- Tool results are hidden from the user and are never an assistant answer, even if a continuation presents one like a prior assistant message. After the tool returns, you must always send a final answer before waiting for more user input.\n- Answer as a concise natural-language sentence in the user's language and requested precision. For a German question, answer only in German; use the form `Heute ist der TT. Monatsname JJJJ.` for a full date or `Monatsname JJJJ.` when only month and year were requested.\n- Never expose tool protocol, JSON, field labels, internal reasoning, or translations the user did not request. Do not reply with a generic greeting or readiness statement.\n\n# Live web research\n- Check the available tools before answering questions about your capabilities.\n- When `mcp__executor__web_search` is available, you have live Internet search through Executor. Use it whenever the user asks for online research or for current, latest, or otherwise time-sensitive information.\n- Use `mcp__executor__web_fetch` to read a result page when search snippets are insufficient.\n- Use the exact registered tool names. Do not substitute an unregistered shorthand such as `web_search` or `web_fetch`.\n- Never claim that Internet access is unavailable while these tools are available, and never present remembered information as a live search result."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_default_model_for_provider_lmstudio() {
        let result = get_default_model_for_oss_provider(LMSTUDIO_OSS_PROVIDER_ID);
        assert_eq!(result, Some(codex_lmstudio::DEFAULT_OSS_MODEL));
    }

    #[test]
    fn test_get_default_model_for_provider_ollama() {
        let result = get_default_model_for_oss_provider(OLLAMA_OSS_PROVIDER_ID);
        assert_eq!(result, Some(codex_ollama::DEFAULT_OSS_MODEL));
    }

    #[test]
    fn test_get_default_model_for_provider_unknown() {
        let result = get_default_model_for_oss_provider("unknown-provider");
        assert_eq!(result, None);
    }

    #[test]
    fn aren_model_info_is_visible_and_does_not_use_fallback_metadata() {
        let metadata = OllamaModelMetadata {
            capabilities: vec!["completion".to_string(), "thinking".to_string()],
            context_window: Some(131_072),
        };

        let model = aren_ollama_model_info("gpt-oss:20b", Some(&metadata), /*priority*/ 0);

        assert_eq!(model.slug, "gpt-oss:20b");
        assert_eq!(model.visibility, ModelVisibility::List);
        assert_eq!(model.default_reasoning_level, Some(ReasoningEffort::High));
        assert_eq!(model.context_window, Some(131_072));
        assert!(!model.used_fallback_model_metadata);
        assert!(!model.base_instructions.contains("OpenAI"));
        assert!(!model.base_instructions.contains("Codex"));
        assert!(model.base_instructions.contains("Aren"));
        assert!(
            model
                .base_instructions
                .contains("phase-by-phase approval checkpoints")
        );
        assert!(
            model
                .base_instructions
                .contains("Never invent an approval requirement")
        );
        assert!(model.base_instructions.contains("local Git repositories"));
        assert!(model.base_instructions.contains("`exec_command`"));
        assert!(model.base_instructions.contains("`<current_date>`"));
        assert!(model.base_instructions.contains("current month"));
        assert!(
            model
                .base_instructions
                .contains("`mcp__executor__get_current_datetime`")
        );
        assert!(
            model
                .base_instructions
                .contains("`mcp__executor__web_search`")
        );
        assert!(
            model
                .base_instructions
                .contains("`mcp__executor__web_fetch`")
        );
    }

    #[test]
    fn normalizes_openai_only_reasoning_levels_for_ollama() {
        assert_eq!(
            normalize_ollama_reasoning_effort(Some(ReasoningEffort::XHigh), true),
            Some(ReasoningEffort::High)
        );
        assert_eq!(
            normalize_ollama_reasoning_effort(Some(ReasoningEffort::Ultra), true),
            Some(ReasoningEffort::Max)
        );
        assert_eq!(
            normalize_ollama_reasoning_effort(Some(ReasoningEffort::High), false),
            Some(ReasoningEffort::None)
        );
    }
}
