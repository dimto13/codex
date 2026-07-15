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
