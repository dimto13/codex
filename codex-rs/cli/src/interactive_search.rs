use anyhow::Context;
use codex_common::CliConfigOverrides;
use codex_common::oss::ensure_oss_provider_ready;
use codex_common::oss::get_default_model_for_oss_provider;
use codex_core::AuthManager;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::LMSTUDIO_OSS_PROVIDER_ID;
use codex_core::OLLAMA_OSS_PROVIDER_ID;
use codex_core::auth::enforce_login_restrictions;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_as_toml_with_cli_overrides;
use codex_core::config::resolve_oss_provider;
use codex_core::get_platform_sandbox;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SessionSource;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::user_input::UserInput;
use codex_tui::Cli as TuiCli;
use codex_utils_absolute_path::AbsolutePathBuf;
use regex_lite::Regex;
use serde_json::json;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::time;

pub(crate) async fn run_interactive_search(
    mut cli: TuiCli,
    json_output: bool,
    timeout_secs: Option<u64>,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let prompt = cli.prompt.clone().unwrap_or_default();
    if prompt.trim().is_empty() {
        anyhow::bail!("interactive-search requires a non-empty prompt");
    }

    // Force web search on for this mode.
    cli.web_search = true;
    cli.config_overrides
        .raw_overrides
        .push("features.web_search_request=true".to_string());

    let (sandbox_mode, approval_policy) = if cli.full_auto {
        (
            Some(SandboxMode::WorkspaceWrite),
            Some(AskForApproval::OnRequest),
        )
    } else if cli.dangerously_bypass_approvals_and_sandbox {
        (
            Some(SandboxMode::DangerFullAccess),
            Some(AskForApproval::Never),
        )
    } else {
        (
            cli.sandbox_mode.map(Into::<SandboxMode>::into),
            cli.approval_policy.map(Into::into),
        )
    };

    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    let overrides_cli = CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = overrides_cli
        .parse_overrides()
        .map_err(|err| anyhow::anyhow!("Error parsing -c overrides: {err}"))?;

    let codex_home = find_codex_home().context("Error finding Codex home")?;
    let cwd = cli.cwd.clone();
    let config_cwd = match cwd.as_deref() {
        Some(path) => AbsolutePathBuf::from_absolute_path(path.canonicalize()?)?,
        None => AbsolutePathBuf::current_dir()?,
    };

    let config_toml =
        load_config_as_toml_with_cli_overrides(&codex_home, &config_cwd, cli_kv_overrides.clone())
            .await
            .context("Error loading config.toml")?;

    let model_provider_override = if cli.oss {
        let resolved = resolve_oss_provider(
            cli.oss_provider.as_deref(),
            &config_toml,
            cli.config_profile.clone(),
        );
        if let Some(provider) = resolved {
            Some(provider)
        } else {
            anyhow::bail!(
                "No default OSS provider configured. Use --local-provider=provider or set oss_provider to either {LMSTUDIO_OSS_PROVIDER_ID} or {OLLAMA_OSS_PROVIDER_ID} in config.toml"
            );
        }
    } else {
        None
    };

    let model = if let Some(model) = &cli.model {
        Some(model.clone())
    } else if cli.oss {
        model_provider_override
            .as_ref()
            .and_then(|provider_id| get_default_model_for_oss_provider(provider_id))
            .map(std::borrow::ToOwned::to_owned)
    } else {
        None
    };

    let overrides = ConfigOverrides {
        model,
        approval_policy,
        sandbox_mode,
        cwd,
        model_provider: model_provider_override.clone(),
        config_profile: cli.config_profile.clone(),
        codex_linux_sandbox_exe,
        show_raw_agent_reasoning: cli.oss.then_some(true),
        additional_writable_roots: cli.add_dir.clone(),
        ..Default::default()
    };

    let config = Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides)
        .await
        .context("Error loading configuration")?;

    if let Some(warning) = add_dir_warning_message(&cli.add_dir, config.sandbox_policy.get()) {
        anyhow::bail!("{warning}");
    }

    if should_show_trust_screen(&config) {
        anyhow::bail!(
            "This directory is not trusted yet. Run the interactive CLI to approve it first."
        );
    }

    if let Err(err) = enforce_login_restrictions(&config).await {
        anyhow::bail!("{err}");
    }

    if cli.oss
        && let Some(provider_id) = model_provider_override.as_ref()
    {
        ensure_oss_provider_ready(provider_id, &config).await?;
    }

    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    let conversation_manager = ConversationManager::new(auth_manager, SessionSource::Cli);
    let new_conversation = conversation_manager
        .new_conversation(config)
        .await
        .context("Failed to initialize Codex conversation")?;
    let model_name = new_conversation.session_configured.model.clone();

    let items = build_user_inputs(prompt, cli.images);
    let timeout = timeout_secs.map(Duration::from_secs);
    let answer =
        run_headless_session(new_conversation.conversation, items, json_output, timeout).await?;

    if json_output {
        let sources = extract_sources(&answer);
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp().to_string());
        let payload = json!({
            "answer": answer,
            "sources": sources,
            "timestamp": timestamp,
            "model": model_name,
        });
        println!("{}", serde_json::to_string(&payload)?);
    } else if !answer.is_empty() {
        println!("{answer}");
    }

    Ok(())
}

async fn run_headless_session(
    conversation: Arc<CodexConversation>,
    items: Vec<UserInput>,
    json_output: bool,
    timeout: Option<Duration>,
) -> anyhow::Result<String> {
    conversation.submit(Op::UserInput { items }).await?;

    let mut last_agent_message: Option<String> = None;
    let mut error_seen = false;
    let mut shutdown_requested = false;
    let mut task_complete = false;

    let mut timeout_sleep = timeout.map(time::sleep);
    tokio::pin!(timeout_sleep);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = conversation.submit(Op::Interrupt).await;
                if !shutdown_requested {
                    let _ = conversation.submit(Op::Shutdown).await;
                    shutdown_requested = true;
                }
            }
            _ = &mut timeout_sleep, if timeout_sleep.is_some() => {
                if !shutdown_requested {
                    let _ = conversation.submit(Op::Shutdown).await;
                }
                let secs = timeout.map_or(0, Duration::as_secs);
                anyhow::bail!("interactive-search timed out after {secs}s");
            }
            event = conversation.next_event() => {
                let event = match event {
                    Ok(event) => event,
                    Err(err) => return Err(err.into()),
                };
                let Event { id, msg } = event;
                match msg {
                    EventMsg::AgentMessage(ev) => {
                        last_agent_message = Some(ev.message);
                    }
                    EventMsg::TaskComplete(ev) => {
                        if last_agent_message.is_none() {
                            last_agent_message = ev.last_agent_message;
                        }
                        task_complete = true;
                        if !shutdown_requested {
                            conversation.submit(Op::Shutdown).await?;
                            shutdown_requested = true;
                        }
                    }
                    EventMsg::TurnAborted(ev) => {
                        anyhow::bail!("task aborted: {:?}", ev.reason);
                    }
                    EventMsg::WebSearchEnd(ev) => {
                        if !json_output {
                            println!("- Searched {}", ev.query);
                        }
                    }
                    EventMsg::ExecApprovalRequest(ev) => {
                        if !json_output {
                            let command = format_command(&ev.command);
                            eprintln!("Approval requested for `{command}`; denying in headless mode.");
                        }
                        conversation
                            .submit(Op::ExecApproval {
                                id,
                                decision: ReviewDecision::Denied,
                            })
                            .await?;
                    }
                    EventMsg::ApplyPatchApprovalRequest(_) => {
                        if !json_output {
                            eprintln!("Patch approval requested; denying in headless mode.");
                        }
                        conversation
                            .submit(Op::PatchApproval {
                                id,
                                decision: ReviewDecision::Denied,
                            })
                            .await?;
                    }
                    EventMsg::ElicitationRequest(ev) => {
                        conversation
                            .submit(Op::ResolveElicitation {
                                server_name: ev.server_name,
                                request_id: ev.id,
                                decision: ElicitationAction::Cancel,
                            })
                            .await?;
                    }
                    EventMsg::Error(ev) => {
                        error_seen = true;
                        if !json_output {
                            eprintln!("error: {}", ev.message);
                        }
                    }
                    EventMsg::Warning(ev) => {
                        if !json_output {
                            eprintln!("warning: {}", ev.message);
                        }
                    }
                    EventMsg::ShutdownComplete => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    if error_seen {
        anyhow::bail!("interactive-search terminated with errors");
    }
    if !task_complete {
        anyhow::bail!("interactive-search terminated before completion");
    }

    Ok(last_agent_message.unwrap_or_default())
}

fn build_user_inputs(prompt: String, images: Vec<PathBuf>) -> Vec<UserInput> {
    let mut items = Vec::new();
    if !prompt.is_empty() {
        items.push(UserInput::Text { text: prompt });
    }
    for path in images {
        items.push(UserInput::LocalImage { path });
    }
    items
}

fn format_command(command: &[String]) -> String {
    if command.is_empty() {
        "<unknown>".to_string()
    } else {
        command.join(" ")
    }
}

fn extract_sources(answer: &str) -> Vec<String> {
    let url_re = Regex::new(r#"https?://[^\s)\]}'"]+"#).expect("valid url regex");
    let mut seen = HashSet::new();
    let mut sources = Vec::new();
    for match_ in url_re.find_iter(answer) {
        let url = trim_url(match_.as_str());
        if seen.insert(url.to_string()) {
            sources.push(url.to_string());
        }
    }
    sources
}

fn trim_url(url: &str) -> &str {
    url.trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ')' | ']' | '}' | '"' | '\''))
}

fn add_dir_warning_message(
    additional_dirs: &[PathBuf],
    sandbox_policy: &codex_core::protocol::SandboxPolicy,
) -> Option<String> {
    if additional_dirs.is_empty() {
        return None;
    }

    match sandbox_policy {
        codex_core::protocol::SandboxPolicy::ReadOnly => Some(format!(
            "Ignoring --add-dir ({}) because the effective sandbox mode is read-only. Switch to workspace-write or danger-full-access to allow additional writable roots.",
            additional_dirs
                .iter()
                .map(PathBuf::to_string_lossy)
                .collect::<Vec<_>>()
                .join(", ")
        )),
        codex_core::protocol::SandboxPolicy::WorkspaceWrite { .. }
        | codex_core::protocol::SandboxPolicy::DangerFullAccess
        | codex_core::protocol::SandboxPolicy::ExternalSandbox { .. } => None,
    }
}

fn should_show_trust_screen(config: &Config) -> bool {
    if cfg!(target_os = "windows") && get_platform_sandbox().is_none() {
        return false;
    }
    if config.did_user_set_custom_approval_policy_or_sandbox_mode {
        return false;
    }
    config.active_project.trust_level.is_none()
}
