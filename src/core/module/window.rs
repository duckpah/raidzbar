//! Active-window module.
//!
//! Queries niri's `FocusedWindow` over `$NIRI_SOCKET`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Module, ModuleChrome, ModuleView};
use crate::core::niri;
use crate::renderer::primitives::TextStyle;

const DEFAULT_MAX_LENGTH: usize = 80;
const DEFAULT_EMPTY_LABEL: &str = "";

pub struct WindowModule {
    niri_socket: Option<PathBuf>,
    title: String,
    max_length: usize,
    empty_label: String,
    chrome: ModuleChrome,
}

impl WindowModule {
    pub fn new(
        max_length: Option<u32>,
        empty_label: Option<String>,
        chrome: ModuleChrome,
    ) -> Self {
        Self {
            niri_socket: niri::socket_path(),
            title: String::new(),
            max_length: max_length
                .map(|v| v as usize)
                .filter(|v| *v > 0)
                .unwrap_or(DEFAULT_MAX_LENGTH),
            empty_label: empty_label.unwrap_or_else(|| DEFAULT_EMPTY_LABEL.to_string()),
            chrome,
        }
    }

    fn refresh(&mut self) {
        self.title = self
            .niri_socket
            .as_deref()
            .and_then(read_niri_title)
            .unwrap_or_default();
    }
}

impl Module for WindowModule {
    fn update(&mut self) {
        self.refresh();
    }

    fn update_interval(&self) -> Duration {
        self.chrome
            .update_interval
            .unwrap_or(Duration::from_millis(100))
    }

    fn view(&self) -> ModuleView {
        let text = if self.title.is_empty() {
            self.empty_label.clone()
        } else {
            truncate_chars(&self.title, self.max_length)
        };
        self.chrome.apply(ModuleView {
            text,
            style: TextStyle::default(),
            ..Default::default()
        })
    }
}

fn read_niri_title(socket: &Path) -> Option<String> {
    parse_niri_title(&niri::request(socket, "\"FocusedWindow\"")?)
}

/// Pull `title` out of `{"Ok":{"FocusedWindow":{…}}}`. A `null` focused
/// window (empty workspace) yields `None`.
fn parse_niri_title(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let window = value.get("Ok")?.get("FocusedWindow")?;
    if window.is_null() {
        return None;
    }
    window.get("title")?.as_str().map(str::to_string)
}

fn truncate_chars(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::{parse_niri_title, truncate_chars};

    #[test]
    fn parses_niri_focused_window_title() {
        let json = r#"{"Ok":{"FocusedWindow":{"id":3,"title":"kitty","app_id":"kitty"}}}"#;
        assert_eq!(parse_niri_title(json).as_deref(), Some("kitty"));
    }

    #[test]
    fn no_niri_focused_window_returns_none() {
        assert_eq!(parse_niri_title(r#"{"Ok":{"FocusedWindow":null}}"#), None);
    }

    #[test]
    fn truncates_with_ellipsis() {
        assert_eq!(truncate_chars("abcdef", 4), "abc…");
        assert_eq!(truncate_chars("abc", 4), "abc");
    }
}
