use std::process::Command;

use serde_json::Value;

/// Returns the default interface
pub fn get_default_interface() -> Result<String, Box<dyn std::error::Error>> {
    let ip = Command::new("ip").args(["-j", "route", "show"]).output()?;

    if !ip.status.success() {
        return Err(format!(
            "commands.rs:get_default_inferface() failed.\nError: {}",
            String::from_utf8_lossy(&ip.stderr)
        )
        .into());
    }
    let json: Value = serde_json::from_str(&String::from_utf8_lossy(&ip.stdout))?;

    // TODO: Handle errors to fix panic, i.e. not arr, empty arr, no dev key
    Ok(json[0]["dev"].to_string())
}
