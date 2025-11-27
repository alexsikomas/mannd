use std::fmt::Debug;

use async_trait::async_trait;
use zbus::Connection;

use crate::{error::ComError, wireless::common::Security};

pub mod agent;
pub mod common;
pub mod iwd;
pub mod wpa_supplicant;
