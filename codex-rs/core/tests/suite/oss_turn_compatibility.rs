use codex_model_provider_info::WireApi;
use codex_model_provider_info::create_oss_provider_with_base_url;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;

const CURRENT_DATETIME_TOOL_NAME: &str = "mcp__executor__get_current_datetime";
const MAX_VISIBLE_OSS_MCP_TOOLS: usize = 12;
const CHROME_INTERACTIVE_SEARCH_TOOL_NAMES: [&str; 4] = [
    "mcp__chrome_devtools__list_pages",
    "mcp__chrome_devtools__new_page",
    "mcp__chrome_devtools__navigate_page",
    "mcp__chrome_devtools__take_snapshot",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oss_current_time_request_requires_executor_tool_then_suppresses_tools()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let call_id = "call-current-datetime";
    let call_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-time-1"),
            responses::ev_function_call(call_id, CURRENT_DATETIME_TOOL_NAME, "{}"),
            responses::ev_completed("resp-time-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-time-1", "Es ist 00:07 Uhr CEST."),
            responses::ev_completed("resp-time-2"),
        ]),
    )
    .await;

    let oss_provider =
        create_oss_provider_with_base_url(&format!("{}/v1", server.uri()), WireApi::Responses);
    let base_test = test_codex()
        .with_config(move |config| config.model_provider = oss_provider)
        .build_with_auto_env(&server)
        .await?;
    let dynamic_tools = vec![
        dynamic_tool(CURRENT_DATETIME_TOOL_NAME),
        dynamic_tool("unrelated_tool"),
    ];
    let new_thread = base_test
        .thread_manager
        .start_thread_with_tools(base_test.config.clone(), dynamic_tools)
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Welche Uhrzeit ist es?".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;

    let EventMsg::DynamicToolCallRequest(request) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::DynamicToolCallRequest(_))
    })
    .await
    else {
        unreachable!("event guard guarantees DynamicToolCallRequest");
    };
    assert_eq!(request.tool, CURRENT_DATETIME_TOOL_NAME);
    assert_eq!(request.arguments, json!({}));

    test.codex
        .submit(Op::DynamicToolResponse {
            id: request.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "Aktuelle Ortszeit: 00:07:00 CEST (UTC offset +0200)".to_string(),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let initial_request = call_mock.single_request();
    let initial_body = initial_request.body_json();
    assert_eq!(initial_body["tool_choice"], "required");
    let initial_tools = initial_body["tools"]
        .as_array()
        .expect("initial OSS request should contain tools");
    assert_eq!(initial_tools.len(), 1);
    assert_eq!(initial_tools[0]["name"], CURRENT_DATETIME_TOOL_NAME);

    let final_request = final_mock.single_request();
    let final_body = final_request.body_json();
    assert_eq!(final_body["tool_choice"], "auto");
    assert_eq!(final_body["tools"], json!([]));
    let final_output = final_request.function_call_output(call_id);
    let output = final_output["output"]
        .as_str()
        .expect("post-tool request should contain text output");
    assert!(output.contains("Aktuelle Ortszeit: 00:07:00 CEST"));
    assert!(output.contains("authoritative current local time"));
    assert!(output.contains("do not call another tool"));
    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oss_large_mcp_catalog_keeps_local_shell_visible_and_capped() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let call_id = "call-local-git";
    let call_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-local-git-1"),
            responses::ev_function_call(
                call_id,
                "exec_command",
                &serde_json::to_string(&json!({
                    "cmd": "git log -1 --pretty=%s",
                    "yield_time_ms": 1_000,
                }))?,
            ),
            responses::ev_completed("resp-local-git-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-local-git", "Der letzte Commit wurde geprüft."),
            responses::ev_completed("resp-local-git-2"),
        ]),
    )
    .await;

    let oss_provider =
        create_oss_provider_with_base_url(&format!("{}/v1", server.uri()), WireApi::Responses);
    let base_test = test_codex()
        .with_config(move |config| config.model_provider = oss_provider)
        .build_with_auto_env(&server)
        .await?;
    let mut dynamic_tools = (0..40)
        .map(|index| dynamic_tool(&format!("mcp__catalog__weather_{index}")))
        .collect::<Vec<_>>();
    dynamic_tools.push(dynamic_tool("mcp__catalog__commit_message"));
    let new_thread = base_test
        .thread_manager
        .start_thread_with_tools(base_test.config.clone(), dynamic_tools)
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Wie lautet die Commit-Nachricht vom letzten Commit in dem Projekt von hier?"
                    .to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let initial_request = call_mock.single_request();
    let initial_body = initial_request.body_json();
    let initial_tools = initial_body["tools"]
        .as_array()
        .expect("initial OSS request should contain tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    assert!(initial_tools.contains(&"exec_command"));
    assert!(initial_tools.contains(&"mcp__catalog__commit_message"));
    assert!(
        initial_tools
            .iter()
            .filter(|name| name.starts_with("mcp__"))
            .count()
            <= MAX_VISIBLE_OSS_MCP_TOOLS
    );
    assert!(initial_tools.len() < 41);

    let final_request = final_mock.single_request();
    let final_output = final_request.function_call_output(call_id);
    let output = final_output["output"]
        .as_str()
        .expect("post-tool request should contain the local Git command output");
    assert!(!output.is_empty());
    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oss_interactive_search_keeps_chrome_tools_visible_in_large_catalog() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let call_id = "call-chrome-new-page";
    let call_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-browser-search-1"),
            responses::ev_function_call(
                call_id,
                "mcp__chrome_devtools__new_page",
                r#"{"url":"https://example.com"}"#,
            ),
            responses::ev_completed("resp-browser-search-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-browser-search", "Die Quellen wurden geprüft."),
            responses::ev_completed("resp-browser-search-2"),
        ]),
    )
    .await;

    let oss_provider =
        create_oss_provider_with_base_url(&format!("{}/v1", server.uri()), WireApi::Responses);
    let base_test = test_codex()
        .with_config(move |config| config.model_provider = oss_provider)
        .build_with_auto_env(&server)
        .await?;
    let mut dynamic_tools = (0..40)
        .map(|index| dynamic_tool(&format!("mcp__catalog__finance_{index}")))
        .collect::<Vec<_>>();
    dynamic_tools.extend(
        CHROME_INTERACTIVE_SEARCH_TOOL_NAMES
            .iter()
            .map(|name| dynamic_tool(name)),
    );
    let new_thread = base_test
        .thread_manager
        .start_thread_with_tools(base_test.config.clone(), dynamic_tools)
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Recherchiere die aktuellen Finanznachrichten.".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: Some(HashMap::from([(
                "aren_interactive_search".to_string(),
                "true".to_string(),
            )])),
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;

    let EventMsg::DynamicToolCallRequest(request) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::DynamicToolCallRequest(_))
    })
    .await
    else {
        unreachable!("event guard guarantees DynamicToolCallRequest");
    };
    assert_eq!(request.tool, "mcp__chrome_devtools__new_page");
    assert_eq!(request.arguments, json!({"url": "https://example.com"}));

    test.codex
        .submit(Op::DynamicToolResponse {
            id: request.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "Opened https://example.com in Chrome.".to_string(),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let initial_request = call_mock.single_request();
    let initial_body = initial_request.body_json();
    assert_eq!(initial_body["tool_choice"], "required");
    let initial_tools = initial_body["tools"]
        .as_array()
        .expect("initial OSS request should contain tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(initial_tools, ["mcp__chrome_devtools__new_page"]);

    let final_request = final_mock.single_request();
    let final_body = final_request.body_json();
    assert_eq!(final_body["tool_choice"], "auto");
    let visible_tools = final_body["tools"]
        .as_array()
        .expect("post-Chrome OSS request should contain tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    assert!(
        CHROME_INTERACTIVE_SEARCH_TOOL_NAMES
            .iter()
            .all(|chrome_tool| visible_tools.contains(chrome_tool))
    );
    assert!(
        visible_tools
            .iter()
            .filter(|name| name.starts_with("mcp__"))
            .count()
            <= MAX_VISIBLE_OSS_MCP_TOOLS
    );
    server.verify().await;

    Ok(())
}

fn dynamic_tool(name: &str) -> DynamicToolSpec {
    DynamicToolSpec::Function(DynamicToolFunctionSpec {
        name: name.to_string(),
        description: format!("Test tool {name}."),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false,
        }),
        defer_loading: false,
    })
}
