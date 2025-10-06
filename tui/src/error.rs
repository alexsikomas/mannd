use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TuiError {
    #[error("std::io::Error encountered, possible issue with Ratatui draw()")]
    IoError(#[from] io::Error),
}
