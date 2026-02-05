use zbus::Connection;
use zbus_systemd::systemd1::UnitProxy;

use crate::error::ManndError;

pub async fn is_service_active(conn: &Connection, service: String) -> Option<bool> {
    if let Ok(unit) = UnitProxy::new(
        conn,
        format!("/org/freedesktop/systemd1/unit/{}_2eservice", service),
    )
    .await
    {
        if let Ok(status) = unit.active_state().await {
            if status == "active" {
                return Some(true);
            }
        }
    }
    return None;
}

pub async fn restart_networkd(conn: &Connection) -> Result<(), ManndError> {
    if let Ok(unit) = UnitProxy::new(
        conn,
        "/org/freedesktop/systemd1/unit/systemd_2dnetworkd_2eservice",
    )
    .await
    {
        unit.restart("replace".to_string()).await?;
    }
    Ok(())
}
