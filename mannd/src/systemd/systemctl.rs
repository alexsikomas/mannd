use tracing::instrument;
use zbus::{Connection, Proxy, zvariant::OwnedObjectPath};
use zbus_systemd::systemd1::UnitProxy;

use crate::error::ManndError;

const SYSTEMD_BUS: &str = "org.freedesktop.systemd1";
const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";

pub async fn get_system_unit(
    conn: &Connection,
    service: String,
) -> Result<OwnedObjectPath, ManndError> {
    let proxy = Proxy::new(
        conn,
        SYSTEMD_BUS,
        SYSTEMD_PATH,
        format!("{SYSTEMD_BUS}.Manager"),
    )
    .await?;
    let res: Result<OwnedObjectPath, _> =
        proxy.call("GetUnit", &format!("{service}.service")).await;

    match res {
        Ok(path) => Ok(path),
        Err(e) => Err(ManndError::Zbus(e)),
    }
}

pub async fn is_service_active(conn: &Connection, service: impl Into<String>) -> Option<bool> {
    let path = get_system_unit(conn, service.into()).await;
    if path.is_err() {
        return None;
    }
    let path = path.unwrap();

    if let Ok(unit) = UnitProxy::new(conn, path).await {
        if let Ok(status) = unit.active_state().await
            && status == "active"
        {
            return Some(true);
        }
    }
    None
}

pub async fn get_service_path(conn: &Connection, service: impl Into<String>) -> String {
    let path = get_system_unit(conn, service.into()).await;
    if path.is_err() {
        return String::new();
    }

    let path = path.unwrap();
    if let Ok(unit) = UnitProxy::new(conn, path).await {
        if let Ok(frag_path) = unit.fragment_path().await {
            return frag_path;
        }
    }
    String::new()
}

#[instrument(err)]
pub async fn restart_networkd(conn: &Connection) -> Result<(), ManndError> {
    let path = get_system_unit(conn, "systemd-networkd".to_string()).await?;
    if let Ok(unit) = UnitProxy::new(conn, path).await {
        unit.restart("replace".to_string()).await?;
    }
    Ok(())
}
