use std::io;

use thiserror::Error;

use crate::app::AppStateBuilderError;

#[derive(Error, Debug)]
pub enum TuiError {
    #[error("Error encountered creating state.")]
    StateBuilder(#[from] AppStateBuilderError),
    #[error("std::io::Error encountered, possible issue with Ratatui draw()")]
    IoError(#[from] io::Error),
}
