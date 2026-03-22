pub mod menu;
pub mod networkd;
pub mod prompts;
pub mod vpn;
pub mod wifi;

use std::sync::OnceLock;
use std::{fmt::Debug, usize};

use crossterm::event::Event;
use mannd::controller::WifiDaemonType;
use mannd::error::ManndError;
use mannd::state::messages::{Capability, NetworkAction, WifiAction, WireguardAction};

use crate::app::{AppAction, NetworkContext};
use crate::keys::{KeyAction, Keymap};
pub use crate::state::menu::MainMenuSelection;
pub use crate::state::networkd::NetdState;
pub use crate::state::prompts::{PopupType, PromptState};
pub use crate::state::vpn::VpnState;
pub use crate::state::wifi::WifiState;

pub static KEYMAP: OnceLock<Keymap> = OnceLock::new();

#[derive(Debug)]
pub enum StateResult {
    Consumed,
    Command(StateCommand),
    Commands(Vec<StateCommand>),
    Ignored,
}

#[derive(Debug)]
pub enum StateCommand {
    Exit,
    ChangeView(View),
    Back,
    NetworkAction(NetworkAction),
    Prompt(PromptState),
    PopPrompt,
    ClearPrompts,
}

pub struct AppContext<'a> {
    pub net_ctx: &'a NetworkContext,
    pub wifi_daemon: &'a Option<WifiDaemonType>,
    pub vpn_cols: usize,
}

pub struct UiState {
    // kbd input block
    pub should_block: bool,
    pub current_view: View,
    pub prompt_stack: Vec<PromptState>,
    pub vpn_cols: usize,
}

impl<'a> AppContext<'a> {
    pub fn create(
        net_ctx: &'a NetworkContext,
        wifi_daemon: &'a Option<WifiDaemonType>,
        // one-to-one map
        vpn_cols: usize,
    ) -> Self {
        Self {
            net_ctx,
            wifi_daemon,
            vpn_cols,
        }
    }
}

trait Component {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult;
}

impl UiState {
    pub fn new(caps: Capability) -> Result<Self, ManndError> {
        let keymap = Keymap::load_keys()?;

        match KEYMAP.set(keymap) {
            Ok(()) => {}
            Err(_) => {
                tracing::warn!("Keymap has already been initialised");
            }
        }

        Ok(UiState {
            should_block: false,
            current_view: View::main_menu(&caps),
            prompt_stack: vec![],
            vpn_cols: 0,
        })
    }

    pub fn refresh_view(&mut self, caps: &Capability) {
        if let View::MainMenu(_) = &self.current_view {
            self.current_view = View::main_menu(caps);
        }
    }

    pub fn handle_event(
        &mut self,
        event: Event,
        ctx: &AppContext,
        caps: &Capability,
    ) -> Vec<AppAction> {
        if self.should_block {
            return vec![];
        }
        let keymap = KEYMAP.get().unwrap();

        let key_action = match event {
            Event::Key(key) => match keymap.bindings.get(&key) {
                Some(key) => key,
                None => {
                    if let Some(c) = key.code.as_char() {
                        &KeyAction::Char(c)
                    } else {
                        &KeyAction::None
                    }
                }
            },
            _ => &KeyAction::None,
        };

        if key_action == &KeyAction::Escape && !self.prompt_stack.is_empty() {
            self.prompt_stack.pop();
            return vec![];
        }

        if let Some(prompt) = self.prompt_stack.last_mut() {
            let res = prompt.on_key(key_action, ctx);
            match res {
                StateResult::Command(cmd) => {
                    return self.process_commands([cmd], caps);
                }
                StateResult::Commands(cmds) => {
                    return self.process_commands(cmds, caps);
                }
                StateResult::Consumed => return vec![],
                StateResult::Ignored => {}
            }
        }

        let res = self.current_view.on_key(key_action, ctx);
        if let StateResult::Command(cmd) = res {
            return self.process_commands([cmd], caps);
        } else if let StateResult::Commands(cmds) = res {
            return self.process_commands(cmds, caps);
        }
        vec![]
    }

    pub fn process_commands(
        &mut self,
        cmds: impl IntoIterator<Item = StateCommand>,
        caps: &Capability,
    ) -> Vec<AppAction> {
        let mut actions: Vec<AppAction> = vec![];
        for cmd in cmds {
            match cmd {
                StateCommand::Exit => actions.push(AppAction::Exit),
                StateCommand::ChangeView(view) => {
                    self.current_view = view;
                    match self.current_view {
                        // improves usability by fetching any networks
                        // that are currently known in connection
                        View::Wifi(_) => actions.push(AppAction::Network(NetworkAction::Wifi(
                            WifiAction::GetNetworks,
                        ))),
                        View::Vpn(_) => actions.push(AppAction::Network(NetworkAction::Wireguard(
                            WireguardAction::GetInfo,
                        ))),
                        _ => {}
                    }
                }
                StateCommand::Prompt(prompt) => {
                    if let PromptState::Info(info) = &prompt {
                        // removes clutter of general messages
                        // if there is an error
                        if info.kind == PopupType::Error {
                            self.remove_general_prompts();
                        }
                    }
                    self.prompt_stack.push(prompt);
                }
                StateCommand::Back => {
                    self.current_view = View::main_menu(caps);
                }
                StateCommand::NetworkAction(action) => actions.push(AppAction::Network(action)),
                StateCommand::PopPrompt => {
                    if !self.prompt_stack.is_empty() {
                        self.prompt_stack.pop();
                    }
                }
                StateCommand::ClearPrompts => {
                    self.prompt_stack = vec![];
                }
            }
        }
        actions
    }

    fn remove_general_prompts(&mut self) {
        self.prompt_stack.retain(|prompt| match prompt {
            PromptState::Info(info) => info.kind != PopupType::General,
            _ => true,
        });
    }
}

#[derive(Debug)]
pub enum View {
    MainMenu(SelectableList<MainMenuSelection>),
    Wifi(WifiState),
    Vpn(VpnState),
    Networkd(NetdState),
    Config,
}

impl View {
    pub fn main_menu(caps: &Capability) -> Self {
        let mut options: SelectableList<MainMenuSelection> = SelectableList::new(vec![]);
        // possibly no interface
        if caps.wifi_daemon.is_some() {
            options.items.push(MainMenuSelection::Wifi);
        }

        if caps.wireguard.0 {
            options.items.push(MainMenuSelection::Vpn);
        }

        if caps.networkd_active {
            options.items.push(MainMenuSelection::Networkd);
        }

        options
            .items
            .extend([MainMenuSelection::Config, MainMenuSelection::Exit]);
        Self::MainMenu(options)
    }
}

impl Component for View {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        if key == &KeyAction::Escape {
            return match self {
                View::MainMenu(_) => StateResult::Command(StateCommand::Exit),
                _ => StateResult::Command(StateCommand::Back),
            };
        }

        match self {
            View::MainMenu(list) => {
                if list.on_key(key, ctx).is_consumed() {
                    return StateResult::Consumed;
                }

                if key == &KeyAction::Enter {
                    if let Some(selection) = list.selected() {
                        return selection.execute(ctx.wifi_daemon);
                    }
                    return StateResult::Consumed;
                }
            }
            View::Wifi(state) => return state.on_key(key, ctx),
            View::Vpn(state) => return state.on_key(key, ctx),
            View::Networkd(_state) => {}
            View::Config => {}
        }
        StateResult::Ignored
    }
}

impl StateResult {
    pub fn is_consumed(&self) -> bool {
        matches!(self, StateResult::Consumed)
    }
}

// Generic data structure used to keep
// track of menu items
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SelectableList<T> {
    pub items: Vec<T>,
    pub selected_index: usize,
}

impl<T> SelectableList<T> {
    pub fn new(v: Vec<T>) -> Self {
        Self {
            items: v,
            selected_index: 0,
        }
    }
    pub fn selected(&self) -> Option<&T> {
        self.items.get(self.selected_index)
    }

    fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected_index = { self.selected_index + 1 } % self.items.len();
    }

    fn prev(&mut self) {
        if self.items.is_empty() {
            return;
        }

        if self.selected_index == 0 {
            self.selected_index = self.items.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }
}

impl<T> Component for SelectableList<T> {
    fn on_key(&mut self, key: &KeyAction, _ctx: &AppContext) -> StateResult {
        match key {
            KeyAction::Up => {
                self.prev();
                StateResult::Consumed
            }
            KeyAction::Down => {
                self.next();
                StateResult::Consumed
            }
            _ => StateResult::Ignored,
        }
    }
}

impl<T: PartialEq> SelectableList<T> {
    pub fn set(&mut self, item: T) {
        if let Some(index) = self.items.iter().position(|x| *x == item) {
            self.selected_index = index;
        }
    }
}

#[derive(Debug, Default)]
pub struct Cursor {
    pub index: usize,
}

impl Cursor {
    pub fn next(&mut self, len: usize) {
        if len > 0 {
            self.index = (self.index + 1) % len;
        }
    }

    pub fn prev(&mut self, len: usize) {
        if len > 0 {
            self.index = self.index.checked_sub(1).unwrap_or(len - 1);
        }
    }

    pub fn forward_clamped(&mut self, step: usize, len: usize) {
        if len == 0 {
            return;
        }

        self.index = self.index.saturating_add(step).min(len - 1);
    }

    pub fn backward_clamped(&mut self, step: usize) {
        self.index = self.index.saturating_sub(step);
    }
}
