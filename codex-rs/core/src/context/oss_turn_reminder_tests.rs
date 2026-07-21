use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

use super::MAX_LATEST_USER_REQUEST_BYTES;
use super::OssToolRouting;
use super::apply_oss_turn_reminder;
use super::oss_tool_routing;

#[test]
fn caps_repeated_latest_user_request_on_a_utf8_boundary() {
    let user_request = "ä".repeat(MAX_LATEST_USER_REQUEST_BYTES);
    let mut input = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: user_request.clone(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    apply_oss_turn_reminder(&mut input, "2026-07-21");

    let ResponseItem::Message { content, .. } = &input[0] else {
        panic!("input should remain a user message");
    };
    let ContentItem::InputText { text: reminder } = &content[1] else {
        panic!("reminder should be appended as input text");
    };
    let repeated_request = reminder
        .split_once("<latest_user_request>\n")
        .and_then(|(_, remainder)| remainder.split_once("\n</latest_user_request>"))
        .map(|(request, _)| request)
        .expect("reminder should quote the latest real user request");
    assert_eq!(repeated_request.len(), MAX_LATEST_USER_REQUEST_BYTES);
    assert_eq!(
        repeated_request,
        &user_request[..MAX_LATEST_USER_REQUEST_BYTES]
    );
}

#[test]
fn embeds_post_tool_reminder_in_the_function_output() {
    let mut input = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "wie spät ist es aktuell?".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "call-time".to_string(),
            output: FunctionCallOutputPayload::from_text("23:48:43 CEST".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    apply_oss_turn_reminder(&mut input, "2026-07-21");

    assert_eq!(input.len(), 2);
    let ResponseItem::FunctionCallOutput { output, .. } = &input[1] else {
        panic!("function output should remain the last request item");
    };
    let output = output
        .text_content()
        .expect("text tool output should remain text");
    assert!(output.starts_with("23:48:43 CEST\n\n<oss_turn_reminder>\n"));
    assert!(
        output.contains("<latest_user_request>\nwie spät ist es aktuell?\n</latest_user_request>")
    );
    assert!(output.contains("authoritative current local time"));
    assert!(output.contains("do not call another tool"));
}

#[test]
fn requires_executor_datetime_tool_for_a_german_current_time_request() {
    let input = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "Welche Uhrzeit ist es?".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    assert_eq!(
        oss_tool_routing(&input),
        OssToolRouting::Require("mcp__executor__get_current_datetime")
    );
}

#[test]
fn suppresses_more_tools_after_current_time_output() {
    let input = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Wie viel Uhr ist es?".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "call-time".to_string(),
            output: FunctionCallOutputPayload::from_text("23:48:43 CEST".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    assert_eq!(oss_tool_routing(&input), OssToolRouting::Suppress);
}

#[test]
fn keeps_default_tools_for_date_requests() {
    let input = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "Welches Datum haben wir heute?".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    assert_eq!(oss_tool_routing(&input), OssToolRouting::Default);
}
