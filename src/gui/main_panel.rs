use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::{ConfigMessage, Message};

#[derive(Serialize, Deserialize)]
pub struct Panel {
    vpn_list: Vec<PathBuf>,
}

impl Default for Panel {
    fn default() -> Self {
        Self { vpn_list: vec![] }
    }
}

impl Panel {
    pub fn new(vpn_list: &Vec<PathBuf>) -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn render(&mut self, ctx: &egui::Context) -> Vec<Message> {
        let messages = vec![];
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("");
        });
        messages
    }
}
