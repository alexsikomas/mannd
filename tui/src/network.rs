use com::wireless::common::AccessPoint;

#[derive(Debug)]
pub enum NetworkAction {
    Scan,
    ForceIwd,
    ForceWpa,
    ForceWifiNetlink,
}

pub enum NetworkUpdate {
    Select(usize),
    Deselect,
    UpdateAps(Vec<AccessPoint>),
}

pub struct NetworkState {
    pub selected: Option<usize>,
    pub aps: Vec<AccessPoint>,
}
