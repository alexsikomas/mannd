use color_eyre::eyre::Result;
use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal).await?;
    ratatui::restore();

    let mut test = nd_common::nl80211::wireless::Wireless::connect().await?;
    let interface = test.get_interfaces().await?;
    nd_common::nl80211::wireless::Wireless::format_interfaces(&interface);
    nd_common::nl80211::wireless::Wireless::format_station(
        &test.get_station(interface[0].index).await?,
    );
    let bss = test.get_bss(interface[0].index).await?;
    nd_common::nl80211::wireless::Wireless::format_bss(bss);
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
