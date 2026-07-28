//! Shared formatting for user-facing `codex resume` command hints.

use codex_protocol::ThreadId;
use codex_shell_command::parse_command::shlex_join;
use std::ffi::OsStr;
use std::path::Path;

pub fn resume_command(thread_name: Option<&str>, thread_id: Option<ThreadId>) -> Option<String> {
    resume_command_for_cli(current_cli_name(), thread_name, thread_id)
}

/// Formats a resume command using the user-facing CLI name supplied by the caller.
pub fn resume_command_for_cli(
    cli_name: &str,
    thread_name: Option<&str>,
    thread_id: Option<ThreadId>,
) -> Option<String> {
    let resume_target = thread_name
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| thread_id.map(|thread_id| thread_id.to_string()));
    resume_target.map(|target| {
        let needs_double_dash = target.starts_with('-');
        let escaped = shlex_join(&[target]);
        if needs_double_dash {
            format!("{cli_name} resume -- {escaped}")
        } else {
            format!("{cli_name} resume {escaped}")
        }
    })
}

pub fn resume_hint(thread_name: Option<&str>, thread_id: Option<ThreadId>) -> Option<String> {
    resume_hint_for_cli(current_cli_name(), thread_name, thread_id)
}

/// Formats a resume hint using the user-facing CLI name supplied by the caller.
pub fn resume_hint_for_cli(
    cli_name: &str,
    thread_name: Option<&str>,
    thread_id: Option<ThreadId>,
) -> Option<String> {
    let thread_id = thread_id?;
    match thread_name.filter(|name| !name.is_empty()) {
        Some(thread_name) => Some(format!(
            "{cli_name} resume, then select {thread_name} ({thread_id})"
        )),
        None => resume_command_for_cli(cli_name, /*thread_name*/ None, Some(thread_id)),
    }
}

fn current_cli_name() -> &'static str {
    cli_name_for_arg0(std::env::args_os().next().as_deref())
}

fn cli_name_for_arg0(arg0: Option<&OsStr>) -> &'static str {
    if arg0
        .and_then(|value| Path::new(value).file_name())
        .is_some_and(|name| name == "aren")
    {
        "aren"
    } else {
        "codex"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn prefers_name_over_id() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command(Some("my-thread"), Some(thread_id));
        assert_eq!(command, Some("codex resume my-thread".to_string()));
    }

    #[test]
    fn formats_thread_id_when_name_is_missing() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command(/*thread_name*/ None, Some(thread_id));
        assert_eq!(
            command,
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn returns_none_without_a_resume_target() {
        let command = resume_command(/*thread_name*/ None, /*thread_id*/ None);
        assert_eq!(command, None);
    }

    #[test]
    fn quotes_thread_names_when_needed() {
        let command = resume_command(Some("-starts-with-dash"), /*thread_id*/ None);
        assert_eq!(
            command,
            Some("codex resume -- -starts-with-dash".to_string())
        );

        let command = resume_command(Some("two words"), /*thread_id*/ None);
        assert_eq!(command, Some("codex resume 'two words'".to_string()));

        let command = resume_command(Some("quote'case"), /*thread_id*/ None);
        assert_eq!(command, Some("codex resume \"quote'case\"".to_string()));
    }

    #[test]
    fn resume_hint_names_picker_item_with_id() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let hint = resume_hint(Some("my-thread"), Some(thread_id));
        assert_eq!(
            hint,
            Some(
                "codex resume, then select my-thread (123e4567-e89b-12d3-a456-426614174000)"
                    .to_string()
            )
        );
    }

    #[test]
    fn resume_hint_uses_direct_id_command_without_name() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let hint = resume_hint(/*thread_name*/ None, Some(thread_id));
        assert_eq!(
            hint,
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn resume_hint_requires_thread_id() {
        let hint = resume_hint(Some("my-thread"), /*thread_id*/ None);
        assert_eq!(hint, None);
    }

    #[test]
    fn formats_aren_resume_commands() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert_eq!(
            resume_command_for_cli("aren", /*thread_name*/ None, Some(thread_id)),
            Some("aren resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
        assert_eq!(
            resume_hint_for_cli("aren", Some("my-thread"), Some(thread_id)),
            Some(
                "aren resume, then select my-thread (123e4567-e89b-12d3-a456-426614174000)"
                    .to_string()
            )
        );
    }

    #[test]
    fn detects_aren_cli_name_from_arg0() {
        assert_eq!(
            cli_name_for_arg0(Some(OsStr::new("/usr/local/bin/aren"))),
            "aren"
        );
        assert_eq!(
            cli_name_for_arg0(Some(OsStr::new("/usr/local/bin/codex"))),
            "codex"
        );
    }
}
