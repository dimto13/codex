use std::ffi::OsStr;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppBrand {
    Codex,
    Aren,
}

impl AppBrand {
    pub(crate) fn current() -> Self {
        Self::from_arg0(std::env::args_os().next().as_deref())
    }

    pub(crate) fn from_arg0(arg0: Option<&OsStr>) -> Self {
        if arg0
            .and_then(|value| Path::new(value).file_name())
            .is_some_and(|name| name == "aren")
        {
            Self::Aren
        } else {
            Self::Codex
        }
    }

    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "OpenAI Codex",
            Self::Aren => "Aren",
        }
    }

    pub(crate) fn product_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Aren => "Aren",
        }
    }

    pub(crate) fn command_name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Aren => "aren",
        }
    }

    pub(crate) fn is_aren(self) -> bool {
        self == Self::Aren
    }
}

#[cfg(test)]
#[path = "brand_tests.rs"]
mod tests;
