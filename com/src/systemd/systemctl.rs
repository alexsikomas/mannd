use tracing::info;
use zbus::Connection;
use zbus_systemd::systemd1::UnitProxy;

pub struct Systemctl {
    conn: Connection,
}

impl Systemctl {
    pub fn new(conn: zbus::Connection) -> Self {
        Self { conn }
    }

    pub async fn is_iwd_active(&self) -> Option<bool> {
        if let Ok(unit) =
            UnitProxy::new(&self.conn, "/org/freedesktop/systemd1/unit/iwd_2eservice").await
        {
            if let Ok(status) = unit.active_state().await {
                if status == "active" {
                    return Some(true);
                }
            }
        }
        return None;
    }

    pub async fn is_service_active(&self, service: String) -> Option<bool> {
        if let Ok(unit) = UnitProxy::new(
            &self.conn,
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
}
