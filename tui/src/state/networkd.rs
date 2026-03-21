use crate::state::SelectableList;

#[derive(Debug)]
pub struct NetdState {
    pub selection: SelectableList<NetdSelection>,
    pub config_cursor: usize,
}

#[derive(Debug)]
pub enum NetdSelection {
    Configs,
    Create,
}

impl Default for NetdState {
    fn default() -> Self {
        Self {
            selection: SelectableList::new(Self::get_actions()),
            config_cursor: 0,
        }
    }
}

impl NetdState {
    fn get_actions() -> Vec<NetdSelection> {
        vec![NetdSelection::Configs, NetdSelection::Create]
    }
}
