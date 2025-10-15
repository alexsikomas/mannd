#[derive(Debug, Clone)]
pub struct AccessPoint {
    pub ssid: String, // name
    pub security: Security,
    pub known: bool,
    pub connected: bool,
    pub nearby: bool,
}

#[derive(Debug, Clone)]
pub enum Security {
    Open,
    Psk,
    Ieee8021x,
}

impl std::fmt::Display for Security {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Security::Open => write!(f, "Open"),
            Security::Psk => write!(f, "Passphrase"),
            Security::Ieee8021x => write!(f, "802.1X"),
        }
    }
}
