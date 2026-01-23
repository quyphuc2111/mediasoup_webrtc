use rusqlite::{Connection, Result as SqlResult, params};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SavedDevice {
    pub id: Option<i64>,
    pub ip: String,
    pub name: String,
    pub port: u16,
    pub last_used: u64,
}

pub fn get_db_path(_app: &AppHandle) -> PathBuf {
    // Use dirs crate to get app data directory
    // This works across platforms without needing Tauri path features
    let app_data = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".screensharing")))
        .or_else(|| std::env::current_dir().ok().map(|d| d.join("app_data")))
        .expect("Failed to get app data directory");
    
    // Create directory if it doesn't exist
    std::fs::create_dir_all(&app_data).ok();
    
    app_data.join("devices.db")
}

pub fn init_database(app: &AppHandle) -> SqlResult<Connection> {
    let db_path = get_db_path(app);
    
    // Create directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    
    let conn = Connection::open(&db_path)?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ip TEXT NOT NULL,
            name TEXT NOT NULL,
            port INTEGER NOT NULL,
            last_used INTEGER NOT NULL,
            UNIQUE(ip, port)
        )",
        [],
    )?;
    
    Ok(conn)
}

pub fn save_device(conn: &Connection, device: &SavedDevice) -> SqlResult<i64> {
    conn.execute(
        "INSERT OR REPLACE INTO devices (ip, name, port, last_used) 
         VALUES (?1, ?2, ?3, ?4)",
        params![device.ip, device.name, device.port, device.last_used],
    )?;
    
    Ok(conn.last_insert_rowid())
}

pub fn get_all_devices(conn: &Connection) -> SqlResult<Vec<SavedDevice>> {
    let mut stmt = conn.prepare(
        "SELECT id, ip, name, port, last_used FROM devices ORDER BY last_used DESC"
    )?;
    
    let device_iter = stmt.query_map(params![], |row| {
        Ok(SavedDevice {
            id: Some(row.get(0)?),
            ip: row.get(1)?,
            name: row.get(2)?,
            port: row.get(3)?,
            last_used: row.get(4)?,
        })
    })?;
    
    let mut devices = Vec::new();
    for device in device_iter {
        devices.push(device?);
    }
    
    Ok(devices)
}

pub fn delete_device(conn: &Connection, id: i64) -> SqlResult<()> {
    conn.execute("DELETE FROM devices WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn update_device_last_used(conn: &Connection, ip: &str, port: u16) -> SqlResult<()> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    conn.execute(
        "UPDATE devices SET last_used = ?1 WHERE ip = ?2 AND port = ?3",
        params![timestamp, ip, port],
    )?;
    
    Ok(())
}
