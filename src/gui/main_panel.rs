use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::Message;

#[derive(Serialize, Deserialize)]
pub struct Panel {
    wg_list: Vec<PathBuf>,
}

impl Default for Panel {
    fn default() -> Self {
        Self { wg_list: vec![] }
    }
}

impl Panel {
    /// Creates a new instance of `Panel`, taking in
    /// a vector of wg configuration files
    pub fn new(wg_list: &Vec<PathBuf>) -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Render loop to be sent back to the controller in the update loop,
    /// returns a vector of `Message` which is processed by the controller
    ///
    /// Creates a grid view of connections based on `wg_list`
    pub fn render(&mut self, ctx: &egui::Context) -> Vec<Message> {
        let messages = vec![];
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("");
        });
        messages
    }
}
