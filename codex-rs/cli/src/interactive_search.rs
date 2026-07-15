use anyhow::Context;
use codex_arg0::Arg0DispatchPaths;
use codex_config::ConfigLoadOptions;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_toml_with_layer_stack;
use codex_core::config::resolve_oss_provider;
use codex_core_api::AuthManager;
use codex_core_api::CodexThread;
use codex_core_api::ThreadManager;
use codex_core_api::build_models_manager;
use codex_core_api::empty_extension_registry;
use codex_core_api::init_state_db;
use codex_core_api::local_agent_graph_store_from_state_db;
use codex_core_api::resolve_installation_id;
use codex_core_api::thread_store_from_config;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_home::CodexHomeUserInstructionsProvider;
use codex_login::AuthConfig;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_login::enforce_login_restrictions;
use codex_ollama::DEFAULT_OSS_MODEL;
use codex_ollama::OllamaClient;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_tui::Cli as TuiCli;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_oss::ensure_oss_provider_ready;
use codex_utils_oss::get_default_model_for_oss_provider;
use regex_lite::Regex;
use serde_json::json;
use std::collections::HashSet;
use std::future::pending;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::time as tokio_time;

pub(crate) async fn run_interactive_search(
    mut cli: TuiCli,
    json_output: bool,
    timeout_secs: Option<u64>,
    arg0_paths: Arg0DispatchPaths,
) -> anyhow::Result<()> {
    let prompt = cli.prompt.clone().unwrap_or_default();
    if prompt.trim().is_empty() {
        anyhow::bail!("interactive-search requires a non-empty prompt");
    }

    cli.web_search = true;
    cli.config_overrides
        .raw_overrides
        .push("web_search=\"live\"".to_string());

    let cli_kv_overrides = cli
        .config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let codex_home = find_codex_home().context("Error finding Codex home")?;
    let config_cwd = resolve_config_cwd(cli.cwd.as_deref())?;
    let loader_overrides = super::loader_overrides_for_profile(cli.config_profile_v2.as_ref())?;

    let model_provider = if cli.oss {
        let config_toml = load_config_toml_with_layer_stack(
            codex_home.as_path(),
            Some(&config_cwd),
            cli_kv_overrides.clone(),
            ConfigLoadOptions {
                loader_overrides: loader_overrides.clone(),
                strict_config: cli.strict_config,
                ..Default::default()
            },
        )
        .await
        .context("Error loading config.toml")?
        .config_toml;
        let Some(provider) = resolve_oss_provider(cli.oss_provider.as_deref(), &config_toml) else {
            anyhow::bail!(
                "No default OSS provider configured. Use --local-provider=ollama or --local-provider=lmstudio, or set oss_provider in config.toml"
            );
        };
        Some(provider)
    } else {
        None
    };

    let model = cli.model.clone().or_else(|| {
        model_provider
            .as_deref()
            .and_then(get_default_model_for_oss_provider)
            .map(str::to_owned)
    });
    let (sandbox_mode, approval_policy) = if cli.dangerously_bypass_approvals_and_sandbox {
        (
            Some(codex_protocol::config_types::SandboxMode::DangerFullAccess),
            Some(AskForApproval::Never),
        )
    } else {
        (
            cli.sandbox_mode.map(Into::into),
            cli.approval_policy.map(Into::into),
        )
    };
    let overrides = ConfigOverrides {
        model,
        approval_policy,
        sandbox_mode,
        cwd: cli.cwd.clone(),
        model_provider: model_provider.clone(),
        codex_self_exe: arg0_paths.codex_self_exe.clone(),
        codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe.clone(),
        main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe.clone(),
        show_raw_agent_reasoning: cli.oss.then_some(true),
        ephemeral: Some(true),
        bypass_hook_trust: cli.bypass_hook_trust.then_some(true),
        additional_writable_roots: cli.add_dir.clone(),
        ..Default::default()
    };
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .cli_overrides(cli_kv_overrides)
        .harness_overrides(overrides)
        .loader_overrides(loader_overrides)
        .strict_config(cli.strict_config)
        .build()
        .await
        .context("Error loading configuration")?;

    if let Some(warning) = add_dir_warning_message(
        &cli.add_dir,
        &config.permissions.effective_permission_profile(),
        config.cwd.as_path(),
    ) {
        anyhow::bail!(warning);
    }
    if config.active_project.trust_level.is_none() {
        anyhow::bail!(
            "This directory is not trusted yet. Run the interactive CLI to approve it first."
        );
    }

    set_default_client_residency_requirement(config.enforce_residency.value());
    enforce_login_restrictions(&AuthConfig {
        codex_home: config.codex_home.to_path_buf(),
        auth_credentials_store_mode: config.cli_auth_credentials_store_mode,
        keyring_backend_kind: config.auth_keyring_backend_kind(),
        forced_login_method: config.forced_login_method,
        forced_chatgpt_workspace_id: config.forced_chatgpt_workspace_id.clone(),
        chatgpt_base_url: Some(config.chatgpt_base_url.clone()),
        auth_route_config: config.auth_route_config(),
    })
    .await?;

    if let Some(provider_id) = model_provider.as_deref() {
        ensure_oss_provider_ready(provider_id, &config).await?;
        if provider_id == "ollama" {
            configure_ollama_reasoning(&mut config).await?;
        }
    }

    let auth_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await;
    let state_db = init_state_db(&config).await;
    let runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        arg0_paths.codex_self_exe,
        arg0_paths.codex_linux_sandbox_exe,
    )?;
    let environment_manager = Arc::new(
        EnvironmentManager::from_codex_home(config.codex_home.clone(), Some(runtime_paths)).await?,
    );
    let thread_store = thread_store_from_config(&config, state_db.clone());
    let installation_id = resolve_installation_id(&config.codex_home).await?;
    let thread_manager = ThreadManager::new(
        &config,
        Arc::clone(&auth_manager),
        build_models_manager(&config, auth_manager),
        SessionSource::Cli,
        environment_manager,
        empty_extension_registry(),
        Arc::new(CodexHomeUserInstructionsProvider::new(
            config.codex_home.clone(),
        )),
        /*analytics_events_client*/ None,
        thread_store,
        local_agent_graph_store_from_state_db(state_db.as_ref()),
        installation_id,
        /*attestation_provider*/ None,
        /*external_time_provider*/ None,
    );
    let new_thread = thread_manager
        .start_thread(config)
        .await
        .context("Failed to initialize Codex thread")?;
    let model_name = new_thread.session_configured.model.clone();
    let thread_id = new_thread.thread_id;
    let thread = new_thread.thread;

    let answer_result = run_headless_session(
        Arc::clone(&thread),
        build_user_inputs(prompt, cli.images.clone()),
        json_output,
        timeout_secs.map(Duration::from_secs),
    )
    .await;
    let shutdown_result = thread.shutdown_and_wait().await;
    let _removed = thread_manager.remove_thread(&thread_id).await;
    let answer = match (answer_result, shutdown_result) {
        (Ok(answer), Ok(())) => answer,
        (Err(err), _) => return Err(err),
        (Ok(_), Err(err)) => return Err(err).context("Failed to shut down Codex thread"),
    };

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

fn resolve_config_cwd(cwd: Option<&Path>) -> anyhow::Result<AbsolutePathBuf> {
    match cwd {
        Some(path) => AbsolutePathBuf::from_absolute_path(path.canonicalize()?).map_err(Into::into),
        None => AbsolutePathBuf::current_dir().map_err(Into::into),
    }
}

async fn configure_ollama_reasoning(config: &mut codex_core::config::Config) -> anyhow::Result<()> {
    let model = config.model.as_deref().unwrap_or(DEFAULT_OSS_MODEL);
    let client = OllamaClient::try_from_oss_provider(config).await?;
    let capabilities = client.fetch_model_capabilities(model).await?;
    let supports_thinking = capabilities
        .iter()
        .any(|capability| capability == "thinking");
    config.model_reasoning_effort =
        normalize_ollama_reasoning_effort(config.model_reasoning_effort.take(), supports_thinking);
    Ok(())
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

async fn run_headless_session(
    thread: Arc<CodexThread>,
    items: Vec<UserInput>,
    json_output: bool,
    timeout: Option<Duration>,
) -> anyhow::Result<String> {
    let turn_id = thread.submit(items.into()).await?;
    let timeout_future = async move {
        match timeout {
            Some(duration) => tokio_time::sleep(duration).await,
            None => pending::<()>().await,
        }
    };
    tokio::pin!(timeout_future);

    let mut last_agent_message = None;
    let mut last_error = None;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = thread.submit(Op::Interrupt).await;
                anyhow::bail!("interactive-search interrupted");
            }
            _ = &mut timeout_future => {
                let _ = thread.submit(Op::Interrupt).await;
                let timeout_secs = timeout.map_or(0, |duration| duration.as_secs());
                anyhow::bail!("interactive-search timed out after {timeout_secs}s");
            }
            event = thread.next_event() => {
                let event = event?;
                if event.id != turn_id {
                    continue;
                }
                match event.msg {
                    EventMsg::AgentMessage(event) => {
                        last_agent_message = Some(event.message);
                    }
                    EventMsg::TurnComplete(event) => {
                        if let Some(error) = event.error.or(last_error) {
                            anyhow::bail!(error.message);
                        }
                        return Ok(last_agent_message.or(event.last_agent_message).unwrap_or_default());
                    }
                    EventMsg::TurnAborted(event) => {
                        anyhow::bail!("task aborted: {:?}", event.reason);
                    }
                    EventMsg::WebSearchEnd(event)
                        if !json_output => {
                            println!("- Searched {}", event.query);
                        }
                    EventMsg::ExecApprovalRequest(event) => {
                        if !json_output {
                            let command = format_command(&event.command);
                            eprintln!("Approval requested for `{command}`; denying in headless mode.");
                        }
                        thread
                            .submit(Op::ExecApproval {
                                id: event.effective_approval_id(),
                                turn_id: Some(event.turn_id),
                                decision: ReviewDecision::Denied,
                            })
                            .await?;
                    }
                    EventMsg::ApplyPatchApprovalRequest(event) => {
                        if !json_output {
                            eprintln!("Patch approval requested; denying in headless mode.");
                        }
                        thread
                            .submit(Op::PatchApproval {
                                id: event.call_id,
                                decision: ReviewDecision::Denied,
                            })
                            .await?;
                    }
                    EventMsg::ElicitationRequest(event) => {
                        thread
                            .submit(Op::ResolveElicitation {
                                server_name: event.server_name,
                                request_id: event.id,
                                decision: ElicitationAction::Cancel,
                                content: None,
                                meta: None,
                            })
                            .await?;
                    }
                    EventMsg::RequestUserInput(_) => {
                        anyhow::bail!("interactive-search cannot answer an interactive user-input request");
                    }
                    EventMsg::RequestPermissions(_) => {
                        anyhow::bail!("interactive-search cannot answer an interactive permissions request");
                    }
                    EventMsg::Error(event) => {
                        if !json_output {
                            eprintln!("error: {}", event.message);
                        }
                        last_error = Some(event);
                    }
                    EventMsg::Warning(event) | EventMsg::GuardianWarning(event)
                        if !json_output => {
                            eprintln!("warning: {}", event.message);
                        }
                    _ => {}
                }
            }
        }
    }
}

fn build_user_inputs(prompt: String, images: Vec<PathBuf>) -> Vec<UserInput> {
    let mut items = images
        .into_iter()
        .map(|path| UserInput::LocalImage { path, detail: None })
        .collect::<Vec<_>>();
    items.push(UserInput::Text {
        text: prompt,
        text_elements: Vec::new(),
    });
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
    let Ok(url_re) = Regex::new(r#"https?://[^\s)\]}'"]+"#) else {
        return Vec::new();
    };
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
    url.trim_end_matches(['.', ',', ';', ')', ']', '}', '"', '\''])
}

fn add_dir_warning_message(
    additional_dirs: &[PathBuf],
    permission_profile: &PermissionProfile,
    cwd: &Path,
) -> Option<String> {
    if additional_dirs.is_empty()
        || matches!(
            permission_profile,
            PermissionProfile::Disabled | PermissionProfile::External { .. }
        )
    {
        return None;
    }

    let file_system_policy = permission_profile.file_system_sandbox_policy();
    if file_system_policy.has_full_disk_write_access()
        || file_system_policy.can_write_path_with_cwd(cwd, cwd)
    {
        return None;
    }

    let joined_paths = additional_dirs
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "Ignoring --add-dir ({joined_paths}) because the effective permissions do not allow additional writable roots. Switch to workspace-write or danger-full-access to allow them."
    ))
}

#[cfg(test)]
#[path = "interactive_search_tests.rs"]
mod tests;
