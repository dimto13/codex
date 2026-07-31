//! Verifies that the agent retries when the SSE stream terminates before
//! delivering a `response.completed` event.

use codex_model_provider_info::WireApi;
use codex_model_provider_info::create_oss_provider_with_base_url;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;

fn sse_incomplete() -> String {
    responses::sse(vec![serde_json::json!({
        "type": "response.output_item.done",
    })])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oss_provider_retries_on_early_close_with_extended_budget() {
    skip_if_no_network!();

    let incomplete_sse = sse_incomplete();
    let completed_sse = responses::sse_completed("resp_ok");

    let (server, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: incomplete_sse,
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: completed_sse,
        }],
    ])
    .await;

    // Disable request-layer retries so the recovery specifically exercises the OSS stream retry
    // default rather than retrying inside the HTTP client.
    let mut model_provider =
        create_oss_provider_with_base_url(&format!("{}/v1", server.uri()), WireApi::Responses);
    model_provider.request_max_retries = Some(0);
    model_provider.stream_idle_timeout_ms = Some(2_000);

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
        })
        .build_with_streaming_server(&server)
        .await
        .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await
        .unwrap();

    wait_for_event(&codex, |event| {
        matches!(
            event,
            EventMsg::StreamError(error) if error.message == "Reconnecting... 1/34"
        )
    })
    .await;

    // Wait until TurnComplete (should succeed after retry).
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let requests = server.requests().await;
    assert_eq!(
        requests.len(),
        2,
        "expected retry after incomplete SSE stream"
    );

    server.shutdown().await;
}
