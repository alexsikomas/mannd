use color_eyre::eyre::Result;
use crossterm::event::{self, Event};
use nd_common::wireless::{iwd::Iwd, nl80211::Netlink, wpa_supplicant::WpaSupplicant};
use ratatui::{DefaultTerminal, Frame};

struct App {
    iwd: Option<Iwd>,
    wpa: Option<WpaSupplicant>,
    netlink: Netlink,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal).await?;
    ratatui::restore();

    Ok(())
}

async fn run(mut terminal: DefaultTerminal) -> Result<()> {
    loop {
        terminal.draw(render)?;
        if matches!(event::read()?, Event::Key(_)) {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    frame.render_widget("test", frame.area());
}
