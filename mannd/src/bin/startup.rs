//! Sets up all previous networking state
//! to have continuity between reboots.
//!
//! Is expected to run just once on startup
//! and then terminate
use std::error::Error;

use mannd::controller::Controller;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let controller = Controller::new().await?;
    Ok(())
}
