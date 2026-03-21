use std::path::PathBuf;

use mannd::{state::network::NetworkAction, store::WG_DIR};

use crate::{
    keys::KeyAction,
    state::{AppContext, Component, Cursor, SelectableList, StateCommand, StateResult},
};

#[derive(Debug, PartialEq, Eq)]
pub enum VpnSelection {
    // Connect,
    Toggle,
    Scan,
    Country,
    Filter,
    // isn't a menu option but rather a section
    Files,
}

impl VpnSelection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Toggle => "Toggle",
            Self::Scan => "Scan Files",
            Self::Country => "Get Countries",
            Self::Filter => "Filter",
            Self::Files => "",
        }
    }
}

#[derive(Debug)]
pub struct VpnState {
    pub selection: SelectableList<VpnSelection>,
    pub file_cursor: Cursor,
}

impl Default for VpnState {
    fn default() -> Self {
        Self {
            selection: Self::get_actions(),
            file_cursor: Cursor::default(),
        }
    }
}

impl VpnState {
    fn get_actions() -> SelectableList<VpnSelection> {
        SelectableList::new(vec![
            VpnSelection::Toggle,
            VpnSelection::Scan,
            VpnSelection::Country,
            VpnSelection::Filter,
            VpnSelection::Files,
        ])
    }
}

impl Component for VpnState {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        if let Some(selected) = self.selection.selected() {
            match key {
                KeyAction::Enter => match selected {
                    VpnSelection::Toggle => {
                        return StateResult::Command(StateCommand::NetworkAction(
                            NetworkAction::ToggleWireguard,
                        ));
                    }
                    VpnSelection::Files => {
                        let mut wg_path = PathBuf::from(WG_DIR);
                        if let Some(data) = ctx.net_ctx.wg_ctx.get_index(self.file_cursor.index) {
                            wg_path.push(data.0);
                            return StateResult::Command(StateCommand::NetworkAction(
                                NetworkAction::ConnectWireguard(wg_path),
                            ));
                        }
                    }
                    _ => {}
                },
                KeyAction::Left => match selected {
                    VpnSelection::Files => {
                        self.file_cursor.backward_clamped(1);
                    }
                    VpnSelection::Toggle => {
                        self.selection.selected_index = self.selection.items.len() - 2;
                    }
                    _ => {
                        self.selection.prev();
                    }
                },
                KeyAction::Right => match selected {
                    VpnSelection::Files => {
                        self.file_cursor
                            .forward_clamped(1, ctx.net_ctx.wg_ctx.len());
                    }
                    VpnSelection::Filter => {
                        self.selection.selected_index = 0;
                    }
                    _ => {
                        self.selection.next();
                    }
                },
                KeyAction::Down => {
                    if selected == &VpnSelection::Files {
                        self.file_cursor
                            .forward_clamped(ctx.vpn_cols, ctx.net_ctx.wg_ctx.len());
                    } else {
                        self.selection.set(VpnSelection::Files);
                    }
                }
                KeyAction::Up => {
                    if self.file_cursor.index < ctx.vpn_cols {
                        self.selection.selected_index = 0;
                    } else {
                        self.file_cursor.backward_clamped(ctx.vpn_cols);
                    }
                }
                _ => {}
            }
        }
        StateResult::Consumed
    }
}
