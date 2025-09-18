use color_eyre::eyre::Result;
use com::{controller::Controller, netlink::WirelessNetlink};
use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};

struct App {
    controller: Controller,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal).await?;
    ratatui::restore();
    // let mut netlink = WirelessNetlink::connect().await?;
    // let interface = netlink.get_interfaces().await?;
    // let bss = netlink.get_bss(interface.first().unwrap().index).await?;
    // WirelessNetlink::format_bss(bss);

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
