use bm25::Document;
use bm25::Language;
use bm25::SearchEngineBuilder;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSpec;
use std::collections::HashSet;
use tracing::instrument;

const MCP_TOOL_NAME_PREFIX: &str = "mcp__";
const MAX_VISIBLE_MCP_TOOLS: usize = 12;
const ALWAYS_VISIBLE_MCP_TOOLS: [&str; 2] =
    ["mcp__executor__web_fetch", "mcp__executor__web_search"];

/// Keeps Aren's local tools prominent while retrieving relevant MCP tools for the current request.
#[instrument(
    level = "trace",
    skip_all,
    fields(input_tool_count = tools.len(), query = query.unwrap_or_default())
)]
pub(crate) fn select_oss_request_tools(query: Option<&str>, tools: &[ToolSpec]) -> Vec<ToolSpec> {
    let mcp_tool_count = tools.iter().filter(|tool| is_mcp_tool(tool)).count();
    if mcp_tool_count <= MAX_VISIBLE_MCP_TOOLS {
        return tools.to_vec();
    }

    let mut selected_indices = tools
        .iter()
        .enumerate()
        .filter_map(|(index, tool)| (!is_mcp_tool(tool)).then_some(index))
        .collect::<Vec<_>>();
    let mut selected_index_set = selected_indices.iter().copied().collect::<HashSet<_>>();

    for (index, tool) in tools.iter().enumerate() {
        if ALWAYS_VISIBLE_MCP_TOOLS.contains(&tool.name()) && selected_index_set.insert(index) {
            selected_indices.push(index);
        }
    }

    let remaining_capacity = MAX_VISIBLE_MCP_TOOLS.saturating_sub(
        selected_indices
            .iter()
            .filter(|index| is_mcp_tool(&tools[**index]))
            .count(),
    );
    if remaining_capacity > 0
        && let Some(query) = query.map(str::trim).filter(|query| !query.is_empty())
    {
        let documents = tools
            .iter()
            .enumerate()
            .filter(|(_, tool)| is_mcp_tool(tool))
            .filter_map(|(index, tool)| {
                ToolSearchInfo::from_tool_spec(tool.clone(), None)
                    .map(|info| Document::new(index, info.entry.search_text))
            })
            .collect::<Vec<_>>();
        if !documents.is_empty() {
            let search_engine =
                SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();
            for result in search_engine.search(query, remaining_capacity) {
                let index = result.document.id;
                if selected_index_set.insert(index) {
                    selected_indices.push(index);
                }
            }
        }
    }

    selected_indices
        .into_iter()
        .map(|index| tools[index].clone())
        .collect()
}

fn is_mcp_tool(tool: &ToolSpec) -> bool {
    tool.name().starts_with(MCP_TOOL_NAME_PREFIX)
}

#[cfg(test)]
#[path = "oss_request_tool_selection_tests.rs"]
mod tests;
