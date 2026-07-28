use super::MAX_VISIBLE_MCP_TOOLS;
use super::McpToolSelectionMode;
use super::select_oss_request_tools;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn leaves_small_tool_sets_unchanged() {
    let tools = vec![
        function_tool("exec_command", "Run a local command."),
        function_tool("mcp__executor__get_current_weather", "Get current weather."),
    ];

    assert_eq!(
        select_oss_request_tools(Some("weather"), &tools, McpToolSelectionMode::Default),
        tools
    );
}

#[test]
fn keeps_core_tools_and_caps_large_mcp_catalogs_by_relevance() {
    let mut tools = vec![
        function_tool("exec_command", "Run a command in the local workspace."),
        function_tool("write_stdin", "Continue a local command."),
    ];
    tools.extend((0..40).map(|index| {
        function_tool(
            &format!("mcp__catalog__weather_{index}"),
            "Look up a weather forecast.",
        )
    }));
    tools.push(function_tool(
        "mcp__catalog__commit_message",
        "Read commit messages from a repository.",
    ));

    let selected = select_oss_request_tools(
        Some("commit message repository"),
        &tools,
        McpToolSelectionMode::Default,
    );
    let names = selected.iter().map(ToolSpec::name).collect::<Vec<_>>();

    assert_eq!(&names[..2], ["exec_command", "write_stdin"]);
    assert!(names.contains(&"mcp__catalog__commit_message"));
    assert!(selected.len() <= 2 + MAX_VISIBLE_MCP_TOOLS);
}

#[test]
fn keeps_executor_web_tools_available_even_for_non_english_requests() {
    let mut tools = vec![function_tool(
        "exec_command",
        "Run a command in the local workspace.",
    )];
    tools.extend((0..40).map(|index| {
        function_tool(
            &format!("mcp__catalog__finance_{index}"),
            "Analyze financial market data.",
        )
    }));
    tools.push(function_tool(
        "mcp__executor__web_fetch",
        "Fetch a web page.",
    ));
    tools.push(function_tool(
        "mcp__executor__web_search",
        "Search the live Internet.",
    ));

    let selected = select_oss_request_tools(
        Some("Kannst du online recherchieren?"),
        &tools,
        McpToolSelectionMode::Default,
    );
    let names = selected.iter().map(ToolSpec::name).collect::<Vec<_>>();

    assert!(names.contains(&"mcp__executor__web_fetch"));
    assert!(names.contains(&"mcp__executor__web_search"));
    assert!(selected.len() <= 1 + MAX_VISIBLE_MCP_TOOLS);
}

#[test]
fn keeps_chrome_tools_available_for_interactive_search() {
    let mut tools = vec![function_tool(
        "exec_command",
        "Run a command in the local workspace.",
    )];
    tools.extend((0..40).map(|index| {
        function_tool(
            &format!("mcp__catalog__finance_{index}"),
            "Analyze financial market data.",
        )
    }));
    let chrome_tools = [
        "mcp__chrome_devtools__list_pages",
        "mcp__chrome_devtools__new_page",
        "mcp__chrome_devtools__navigate_page",
        "mcp__chrome_devtools__take_snapshot",
    ];
    tools.extend(
        chrome_tools
            .iter()
            .map(|name| function_tool(name, "Inspect a web page in Chrome.")),
    );

    let selected = select_oss_request_tools(
        Some("Recherchiere die aktuellen Nachrichten"),
        &tools,
        McpToolSelectionMode::InteractiveSearch,
    );
    let names = selected.iter().map(ToolSpec::name).collect::<Vec<_>>();

    assert!(
        chrome_tools
            .iter()
            .all(|chrome_tool| names.contains(chrome_tool))
    );
    assert!(selected.len() <= 1 + MAX_VISIBLE_MCP_TOOLS);
}

fn function_tool(name: &str, description: &str) -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: name.to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
        output_schema: None,
    })
}
