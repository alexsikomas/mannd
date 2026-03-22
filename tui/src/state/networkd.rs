use crate::state::SelectableList;

#[derive(Debug)]
pub struct NetworkdState {
    pub selection: SelectableList<NetworkdSelection>,
    pub config_cursor: usize,
}

#[derive(Debug)]
pub enum NetworkdSelection {
    Configs,
    Create,
}

impl Default for NetworkdState {
    fn default() -> Self {
        Self {
            selection: SelectableList::new(Self::get_actions()),
            config_cursor: 0,
        }
    }
}

impl NetworkdState {
    fn get_actions() -> Vec<NetworkdSelection> {
        vec![NetworkdSelection::Configs, NetworkdSelection::Create]
    }
}
