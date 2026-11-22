use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use super::{Module, ModuleChrome, ModuleView, TextSegment};
use crate::core::config::{ModuleConfig, resolve_color};
use crate::core::event::{ClickEvent, MouseButton};
use crate::core::niri;
use crate::renderer::color::Color;
use crate::renderer::primitives::TextStyle;

const ACTIVE_LABEL_COLOR: Color = Color::rgb(0.98, 0.82, 0.53);
const OCCUPIED_LABEL_COLOR: Color = Color::rgb(0.57, 0.84, 0.98);
const EMPTY_LABEL_COLOR: Color = Color::rgb(0.57, 0.60, 0.68);

#[derive(Debug, Clone)]
struct WorkspaceColors {
    active: Color,
    occupied: Color,
    empty: Color,
}

impl WorkspaceColors {
    fn from_config(config: &ModuleConfig) -> Self {
        Self {
            active: resolve_color(
                config.active_color.as_deref(),
                ACTIVE_LABEL_COLOR,
                "module.workspaces.active_color",
            ),
            occupied: resolve_color(
                config.occupied_color.as_deref(),
                OCCUPIED_LABEL_COLOR,
                "module.workspaces.occupied_color",
            ),
            empty: resolve_color(
                config.empty_color.as_deref(),
                EMPTY_LABEL_COLOR,
                "module.workspaces.empty_color",
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct WorkspaceSlot {
    number: u32,
    token: String,
    occupied: bool,
}

#[derive(Debug, Clone)]
struct WorkspaceState {
    active: u32,
    occupied: HashSet<u32>,
}

pub struct WorkspacesModule {
    count: u32,
    labels: Vec<String>,
    chrome: ModuleChrome,
    colors: WorkspaceColors,
    active: u32,
    occupied: HashSet<u32>,
    niri_socket: Option<PathBuf>,
    logged_state_warning: bool,
}

impl WorkspacesModule {
    pub fn new(
        count: u32,
        labels: Vec<String>,
        chrome: ModuleChrome,
        config: &ModuleConfig,
    ) -> Self {
        Self {
            count,
            labels,
            chrome,
            colors: WorkspaceColors::from_config(config),
            active: 1,
            occupied: HashSet::new(),
            niri_socket: niri::socket_path(),
            logged_state_warning: false,
        }
    }

    fn slots(&self) -> Vec<WorkspaceSlot> {
        (1..=self.count)
            .map(|number| {
                let label = self.workspace_label(number);
                let token = if number == self.count {
                    label.clone()
                } else {
                    format!("{label} ")
                };
                WorkspaceSlot {
                    number,
                    token,
                    occupied: self.occupied.contains(&number),
                }
            })
            .collect()
    }

    fn workspace_label(&self, number: u32) -> String {
        self.labels
            .get(number.saturating_sub(1) as usize)
            .filter(|label| !label.is_empty())
            .cloned()
            .unwrap_or_else(|| workspace_label(number))
    }

    fn refresh_workspace_state(&mut self) {
        if let Some(state) = self.read_workspace_state() {
            self.active = state.active;
            self.occupied = state.occupied;
            self.logged_state_warning = false;
            return;
        }

        if !self.logged_state_warning {
            match &self.niri_socket {
                Some(socket) => log::warn!(
                    "[workspaces] could not read niri state from {}; indicators may be stale",
                    socket.display()
                ),
                None => log::warn!(
                    "[workspaces] $NIRI_SOCKET is not set; workspace indicators may be stale"
                ),
            }
            self.logged_state_warning = true;
        }
    }

    fn read_workspace_state(&self) -> Option<WorkspaceState> {
        let socket = self.niri_socket.as_ref()?;
        let workspaces = niri::request(socket, "\"Workspaces\"")?;
        let windows = niri::request(socket, "\"Windows\"")?;
        parse_niri_workspaces(&workspaces, &windows, self.count)
    }

    fn request_workspace_switch(&mut self, number: u32) {
        if number == self.active || !(1..=self.count).contains(&number) {
            return;
        }

        let Some(socket) = self.niri_socket.as_ref() else {
            return;
        };
        let request =
            format!(r#"{{"Action":{{"FocusWorkspace":{{"reference":{{"Index":{number}}}}}}}}}"#);
        match niri::request(socket, &request) {
            Some(_) => self.active = number,
            None => log::warn!(
                "[workspaces] failed to switch to workspace {number} via {}",
                socket.display()
            ),
        }
    }

    fn workspace_for_click(&self, event: &ClickEvent) -> Option<u32> {
        let slots = self.slots();
        if slots.is_empty() {
            return None;
        }

        let view = self.view();
        let left_pad = view.padding.0;
        let right_pad = view.padding.1;
        let content_width = (event.module_width - left_pad - right_pad).max(0.0);
        if content_width <= 0.0 {
            return None;
        }

        let rel_x = event.x - left_pad;
        if !(0.0..content_width).contains(&rel_x) {
            return None;
        }

        let total_chars = slots
            .iter()
            .map(|slot| slot.token.chars().count())
            .sum::<usize>() as f64;
        if total_chars <= 0.0 {
            return None;
        }

        let char_width = content_width / total_chars;
        let mut cursor = 0.0;
        for slot in slots {
            let token_width = slot.token.chars().count() as f64 * char_width;
            if rel_x < cursor + token_width {
                return Some(slot.number);
            }
            cursor += token_width;
        }

        None
    }
}

impl Module for WorkspacesModule {
    fn update(&mut self) {
        self.refresh_workspace_state();
    }

    fn update_interval(&self) -> Duration {
        self.chrome
            .update_interval
            .unwrap_or(Duration::from_millis(100))
    }

    fn view(&self) -> ModuleView {
        let base_style = TextStyle {
            color: self.chrome.foreground.unwrap_or(Color::WHITE),
            ..TextStyle::default()
        };
        let active_style = TextStyle {
            color: self.colors.active,
            ..base_style.clone()
        };
        let occupied_style = TextStyle {
            color: self.colors.occupied,
            ..base_style.clone()
        };
        let empty_style = TextStyle {
            color: self.colors.empty,
            ..base_style.clone()
        };

        let mut text = String::new();
        let mut text_segments = Vec::new();

        for slot in self.slots() {
            text.push_str(&slot.token);
            text_segments.push(TextSegment {
                text: slot.token,
                style: if slot.number == self.active {
                    active_style.clone()
                } else if slot.occupied {
                    occupied_style.clone()
                } else {
                    empty_style.clone()
                },
            });
        }

        self.chrome.apply(ModuleView {
            text,
            text_segments,
            style: base_style,
            ..Default::default()
        })
    }

    fn click(&mut self, event: ClickEvent) {
        if event.button != MouseButton::Left {
            return;
        }

        if let Some(number) = self.workspace_for_click(&event) {
            self.request_workspace_switch(number);
        }
    }
}

fn workspace_label(number: u32) -> String {
    if number == 10 {
        "0".to_string()
    } else {
        number.to_string()
    }
}

/// Parse niri's `Workspaces` and `Windows` replies. Occupancy uses window
/// `workspace_id`s (a workspace can hold windows without a focused one), plus
/// urgent workspaces.
fn parse_niri_workspaces(
    workspaces_json: &str,
    windows_json: &str,
    count: u32,
) -> Option<WorkspaceState> {
    use serde_json::Value;

    let workspaces: Value = serde_json::from_str(workspaces_json).ok()?;
    let windows: Value = serde_json::from_str(windows_json).ok()?;

    let workspace_list = workspaces.get("Ok")?.get("Workspaces")?.as_array()?;
    let window_list = windows.get("Ok")?.get("Windows")?.as_array()?;

    let mut occupied_ids = HashSet::new();
    for window in window_list {
        if let Some(id) = window.get("workspace_id").and_then(Value::as_u64) {
            occupied_ids.insert(id);
        }
    }

    let mut active = None;
    let mut occupied = HashSet::new();
    for workspace in workspace_list {
        let Some(id) = workspace.get("id").and_then(Value::as_u64) else {
            continue;
        };
        let Some(idx) = workspace.get("idx").and_then(Value::as_u64) else {
            continue;
        };
        let idx = idx as u32;
        let is_active = workspace
            .get("is_active")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let is_urgent = workspace
            .get("is_urgent")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if is_active {
            active = Some(idx);
        }
        if (1..=count).contains(&idx) && (occupied_ids.contains(&id) || is_urgent) {
            occupied.insert(idx);
        }
    }

    Some(WorkspaceState {
        // 0 means "none of the displayed slots is active" (e.g. when the
        // active workspace index is beyond the configured count).
        active: active.unwrap_or(0),
        occupied,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{WorkspacesModule, parse_niri_workspaces};
    use crate::core::config::ModuleConfig;
    use crate::core::module::Module;
    use crate::core::module::ModuleChrome;

    fn default_module_config() -> ModuleConfig {
        ModuleConfig {
            kind: "workspaces".to_string(),
            ..Default::default()
        }
    }

    fn default_chrome() -> ModuleChrome {
        ModuleChrome::default()
    }

    #[test]
    fn parses_niri_workspace_state() {
        let workspaces = r#"{"Ok":{"Workspaces":[
            {"id":2,"idx":2,"name":null,"is_urgent":false,"is_active":false,"active_window_id":null},
            {"id":1,"idx":1,"name":null,"is_urgent":false,"is_active":true,"active_window_id":7}
        ]}}"#;
        let windows = r#"{"Ok":{"Windows":[
            {"id":7,"workspace_id":1},
            {"id":8,"workspace_id":2}
        ]}}"#;

        let state = parse_niri_workspaces(workspaces, windows, 5).unwrap();
        assert_eq!(state.active, 1);
        assert_eq!(state.occupied, HashSet::from([1, 2]));
    }

    #[test]
    fn renders_plain_workspace_numbers() {
        let mut module =
            WorkspacesModule::new(4, Vec::new(), default_chrome(), &default_module_config());
        module.active = 2;
        module.occupied = HashSet::from([2, 4]);

        let view = module.view();
        let rendered_tokens = view
            .text_segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>();

        assert_eq!(view.text, "1 2 3 4");
        assert_eq!(rendered_tokens, vec!["1 ", "2 ", "3 ", "4"]);
    }

    #[test]
    fn uses_configured_workspace_labels() {
        let mut module = WorkspacesModule::new(
            4,
            vec![
                "α".to_string(),
                "β".to_string(),
                "γ".to_string(),
                "δ".to_string(),
            ],
            default_chrome(),
            &default_module_config(),
        );
        module.active = 1;

        let view = module.view();
        let rendered_tokens = view
            .text_segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>();

        assert_eq!(view.text, "α β γ δ");
        assert_eq!(rendered_tokens, vec!["α ", "β ", "γ ", "δ"]);
    }
}
