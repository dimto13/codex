use super::*;
use pretty_assertions::assert_eq;

#[test]
fn build_user_inputs_places_images_before_prompt() {
    let image = PathBuf::from("chart.png");

    let actual = build_user_inputs("analyze this".to_string(), vec![image.clone()]);

    assert_eq!(
        actual,
        vec![
            UserInput::LocalImage {
                path: image,
                detail: None,
            },
            UserInput::Text {
                text: "analyze this".to_string(),
                text_elements: Vec::new(),
            },
        ]
    );
}

#[test]
fn oss_interactive_search_adds_live_tool_guidance() {
    assert_eq!(
        interactive_search_context(true),
        BTreeMap::from([(
            "interactive_search_tools".to_string(),
            AdditionalContextEntry {
                value: "For every Internet research request in this interactive-search turn, you must use the connected Chrome DevTools MCP whenever its `mcp__chrome_devtools__*` tools are available. Use `mcp__chrome_devtools__new_page` or `mcp__chrome_devtools__navigate_page` at least once to open a relevant page and inspect its contents with `mcp__chrome_devtools__take_snapshot`. Built-in search and Executor web tools may be used to discover candidate URLs, but do not answer from those results alone; verify important claims in Chrome. Local Executor MCP tools have names beginning with `mcp__executor__`; for current weather, call `mcp__executor__get_current_weather` when it is available. Do not claim that live access is unavailable before checking for and attempting an appropriate tool.".to_string(),
                kind: AdditionalContextKind::Application,
            },
        )])
    );
}

#[test]
fn hosted_interactive_search_adds_chrome_guidance() {
    assert_eq!(
        interactive_search_context(false),
        BTreeMap::from([(
            "interactive_search_tools".to_string(),
            AdditionalContextEntry {
                value: "For every Internet research request in this interactive-search turn, you must use the connected Chrome DevTools MCP whenever its `mcp__chrome_devtools__*` tools are available. Use `mcp__chrome_devtools__new_page` or `mcp__chrome_devtools__navigate_page` at least once to open a relevant page and inspect its contents with `mcp__chrome_devtools__take_snapshot`. Built-in search may be used to discover candidate URLs, but do not answer from those results alone; verify important claims in Chrome. Do not claim that live access is unavailable before checking for and attempting an appropriate tool.".to_string(),
                kind: AdditionalContextKind::Application,
            },
        )])
    );
}

#[test]
fn chrome_tool_approval_is_accepted_in_headless_search() {
    let request = ElicitationRequest::Form {
        meta: Some(json!({
            APPROVAL_KIND_KEY: APPROVAL_KIND_MCP_TOOL_CALL,
        })),
        message: "Allow Chrome DevTools to open a page?".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": {},
        }),
    };

    assert_eq!(
        interactive_search_elicitation_decision("chrome-devtools", &request),
        ElicitationAction::Accept
    );
    assert_eq!(
        interactive_search_elicitation_decision("chrome_devtools", &request),
        ElicitationAction::Accept
    );
}

#[test]
fn non_chrome_tool_approval_is_cancelled_in_headless_search() {
    let request = ElicitationRequest::Form {
        meta: Some(json!({
            APPROVAL_KIND_KEY: APPROVAL_KIND_MCP_TOOL_CALL,
        })),
        message: "Allow another server to run a tool?".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": {},
        }),
    };

    assert_eq!(
        interactive_search_elicitation_decision("another-server", &request),
        ElicitationAction::Cancel
    );
}

#[test]
fn chrome_form_elicitation_is_cancelled_in_headless_search() {
    let request = ElicitationRequest::Form {
        meta: None,
        message: "Enter a value".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": {
                "value": {
                    "type": "string",
                },
            },
        }),
    };

    assert_eq!(
        interactive_search_elicitation_decision("chrome-devtools", &request),
        ElicitationAction::Cancel
    );
}

#[test]
fn ollama_model_info_uses_reported_context_window() {
    let mut expected = codex_models_manager::model_info::model_info_from_slug("gpt-oss:20b");
    expected.used_fallback_model_metadata = false;
    expected.context_window = Some(131_072);
    expected.max_context_window = Some(131_072);

    assert_eq!(ollama_model_info("gpt-oss:20b", Some(131_072)), expected);
}

#[test]
fn extract_sources_preserves_order_and_removes_duplicates() {
    let actual = extract_sources(
        "First https://example.com/a. Then https://example.org/b, and https://example.com/a.",
    );

    assert_eq!(
        actual,
        vec![
            "https://example.com/a".to_string(),
            "https://example.org/b".to_string(),
        ]
    );
}

#[test]
fn ollama_reasoning_is_disabled_for_models_without_thinking() {
    let actual = normalize_ollama_reasoning_effort(Some(ReasoningEffort::XHigh), false);

    assert_eq!(actual, Some(ReasoningEffort::None));
}

#[test]
fn ollama_reasoning_maps_unsupported_efforts_for_thinking_models() {
    let actual = [
        normalize_ollama_reasoning_effort(None, true),
        normalize_ollama_reasoning_effort(Some(ReasoningEffort::Minimal), true),
        normalize_ollama_reasoning_effort(Some(ReasoningEffort::XHigh), true),
        normalize_ollama_reasoning_effort(Some(ReasoningEffort::Ultra), true),
    ];

    assert_eq!(
        actual,
        [
            Some(ReasoningEffort::High),
            Some(ReasoningEffort::Low),
            Some(ReasoningEffort::High),
            Some(ReasoningEffort::Max),
        ]
    );
}
