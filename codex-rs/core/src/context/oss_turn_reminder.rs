use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseItem;

use super::ContextualUserFragment;
use super::is_contextual_user_fragment;

const MAX_LATEST_USER_REQUEST_BYTES: usize = 2048;
const CURRENT_DATETIME_TOOL_NAME: &str = "mcp__executor__get_current_datetime";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OssTurnPhase {
    Initial,
    AfterTool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OssToolRouting {
    Default,
    Require(&'static str),
    Suppress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OssTurnIntent {
    General,
    CurrentTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OssTurnReminder {
    current_date: String,
    phase: OssTurnPhase,
    intent: OssTurnIntent,
    latest_user_request: Option<String>,
}

impl OssTurnReminder {
    fn new(current_date: &str, phase: OssTurnPhase, latest_user_request: Option<String>) -> Self {
        let intent = latest_user_request
            .as_deref()
            .map(classify_turn_intent)
            .unwrap_or(OssTurnIntent::General);
        Self {
            current_date: current_date.to_string(),
            phase,
            intent,
            latest_user_request,
        }
    }

    fn german_date_renderings(&self) -> Option<(String, String)> {
        let mut parts = self.current_date.split('-');
        let (Some(year), Some(month), Some(day), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        let month_name = match month {
            "01" => "Januar",
            "02" => "Februar",
            "03" => "März",
            "04" => "April",
            "05" => "Mai",
            "06" => "Juni",
            "07" => "Juli",
            "08" => "August",
            "09" => "September",
            "10" => "Oktober",
            "11" => "November",
            "12" => "Dezember",
            _ => return None,
        };
        let day = day.parse::<u8>().ok()?;
        if !(1..=31).contains(&day) || year.len() != 4 {
            return None;
        }
        Some((
            format!("Heute ist der {day}. {month_name} {year}."),
            format!("{month_name} {year}."),
        ))
    }
}

fn turn_phase(input: &[ResponseItem]) -> OssTurnPhase {
    if input.last().is_some_and(|item| {
        matches!(
            item,
            ResponseItem::FunctionCallOutput { .. }
                | ResponseItem::CustomToolCallOutput { .. }
                | ResponseItem::ToolSearchOutput { .. }
        )
    }) {
        OssTurnPhase::AfterTool
    } else {
        OssTurnPhase::Initial
    }
}

fn latest_user_request(input: &[ResponseItem]) -> Option<(usize, String)> {
    let index = input.iter().rposition(|item| {
        matches!(
            item,
            ResponseItem::Message { role, content, .. }
                if role == "user"
                    && content.iter().any(|item| {
                        matches!(item, ContentItem::InputText { .. })
                            && !is_contextual_user_fragment(item)
                    })
        )
    })?;
    let ResponseItem::Message { content, .. } = &input[index] else {
        return None;
    };
    let request = content
        .iter()
        .filter(|item| !is_contextual_user_fragment(item))
        .filter_map(|item| match item {
            ContentItem::InputText { text } => Some(text.as_str()),
            ContentItem::InputImage { .. } | ContentItem::OutputText { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let request = request.trim();
    if request.is_empty() {
        return None;
    }

    let mut end = request.len().min(MAX_LATEST_USER_REQUEST_BYTES);
    while !request.is_char_boundary(end) {
        end -= 1;
    }
    Some((index, request[..end].to_string()))
}

fn classify_turn_intent(request: &str) -> OssTurnIntent {
    let request = request.to_lowercase();
    if [
        "wie spät ist es",
        "wie spaet ist es",
        "wie viel uhr",
        "wieviel uhr",
        "aktuelle uhrzeit",
        "aktuellen uhrzeit",
        "jetzige uhrzeit",
        "welche uhrzeit",
        "what time is it",
        "current time",
        "local time",
    ]
    .iter()
    .any(|phrase| request.contains(phrase))
    {
        OssTurnIntent::CurrentTime
    } else {
        OssTurnIntent::General
    }
}

pub(crate) fn oss_tool_routing(input: &[ResponseItem]) -> OssToolRouting {
    let Some((_, request)) = latest_user_request(input) else {
        return OssToolRouting::Default;
    };
    if classify_turn_intent(&request) != OssTurnIntent::CurrentTime {
        return OssToolRouting::Default;
    }

    match turn_phase(input) {
        OssTurnPhase::Initial => OssToolRouting::Require(CURRENT_DATETIME_TOOL_NAME),
        OssTurnPhase::AfterTool => OssToolRouting::Suppress,
    }
}

pub(crate) fn apply_oss_turn_reminder(input: &mut Vec<ResponseItem>, current_date: &str) {
    let phase = turn_phase(input);
    let latest_user_request = latest_user_request(input);
    let latest_user_message_index = latest_user_request.as_ref().map(|(index, _)| *index);
    let latest_user_request = latest_user_request.map(|(_, request)| request);
    let reminder = OssTurnReminder::new(current_date, phase, latest_user_request).render();

    if phase == OssTurnPhase::AfterTool
        && let Some(
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. },
        ) = input.last_mut()
    {
        if let Some(text) = output.text_content_mut() {
            text.push_str("\n\n");
            text.push_str(&reminder);
        } else if let Some(content) = output.content_items_mut() {
            content.push(FunctionCallOutputContentItem::InputText { text: reminder });
        }
        return;
    }

    if phase == OssTurnPhase::Initial
        && let Some(index) = latest_user_message_index
        && index == input.len() - 1
        && let ResponseItem::Message { content, .. } = &mut input[index]
    {
        content.push(ContentItem::InputText { text: reminder });
        return;
    }

    input.push(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: reminder }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    });
}

impl ContextualUserFragment for OssTurnReminder {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<oss_turn_reminder>\n", "\n</oss_turn_reminder>")
    }

    fn body(&self) -> String {
        let mut lines = vec![format!(
            "System-provided current local date: {}.",
            self.current_date
        )];
        if let Some((full_date, month_year)) = self.german_date_renderings() {
            lines.push(format!("Exact German full-date answer: {full_date}"));
            lines.push(format!("Exact German month-and-year answer: {month_year}"));
        }
        if let Some(latest_user_request) = &self.latest_user_request {
            lines.push(format!(
                "Latest real user request (quoted verbatim):\n<latest_user_request>\n{latest_user_request}\n</latest_user_request>"
            ));
        }
        lines.push(
            "Answer the quoted latest real user request now, in the user's language and requested format."
                .to_string(),
        );
        lines.push(
            "If it asks for today's date, the current month, or the current year, answer directly from the system date above. Do not ask for clarification and do not call a time tool."
                .to_string(),
        );
        if self.phase == OssTurnPhase::AfterTool {
            let instruction = match self.intent {
                OssTurnIntent::General => {
                    "The preceding function output is hidden from the user. Use it to finish the pending request, and do not call the same tool again."
                }
                OssTurnIntent::CurrentTime => {
                    "The preceding function output is the authoritative current local time. Report that time directly in the user's language. Do not claim that realtime information is unavailable and do not call another tool."
                }
            };
            lines.push(instruction.to_string());
        } else if self.intent == OssTurnIntent::CurrentTime {
            lines.push(
                "This request asks for the current local time. Call the provided current-datetime tool now and use its result as the authoritative answer."
                    .to_string(),
            );
        }
        lines.push(
            "Do not greet, ask for a new task, claim there is no pending request, or say changes were applied unless the user asked for changes."
                .to_string(),
        );
        lines.join("\n")
    }
}

#[cfg(test)]
#[path = "oss_turn_reminder_tests.rs"]
mod tests;
