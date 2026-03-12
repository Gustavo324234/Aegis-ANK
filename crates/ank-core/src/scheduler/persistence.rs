use crate::pcb::PCB;
use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tokio::task;
use tracing::{debug, info};

#[async_trait]
pub trait StatePersistor: Send + Sync {
    async fn save_pcb(&self, pcb: &PCB) -> Result<()>;
    async fn delete_pcb(&self, pid: &str) -> Result<()>;
    async fn load_all_pcbs(&self) -> Result<Vec<PCB>>;
    async fn flush(&self) -> Result<()>;
}

pub struct SQLCipherPersistor {
    conn: Arc<Mutex<Connection>>,
}

impl SQLCipherPersistor {
    pub fn new(db_path: &str, key: &str) -> Result<Self> {
        info!(path = %db_path, "Initializing PersistenceManager (SQLCipher).");
        let conn = Connection::open(db_path).context("Failed to open SQLCipher database")?;

        // Aplicamos la llave de cifrado
        conn.pragma_update(None, "key", key)
            .context("Failed to set SQLCipher key")?;

        // Verificamos integridad (esto fallará si la llave es incorrecta)
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .context("SQLCipher authentication failed or database corrupted")?;

        // Esquema atómico para PCBs
        conn.execute(
            "CREATE TABLE IF NOT EXISTS process_control_blocks (
                pid TEXT PRIMARY KEY,
                state TEXT NOT NULL,
                data TEXT NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .context("Failed to initialize PCB table")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl StatePersistor for SQLCipherPersistor {
    async fn save_pcb(&self, pcb: &PCB) -> Result<()> {
        let pcb_clone = pcb.clone();
        let conn = self.conn.clone();

        task::spawn_blocking(move || {
            let json_data = serde_json::to_string(&pcb_clone).context("Failed to serialize PCB")?;
            let lock = conn.lock().map_err(|_| anyhow::anyhow!("Mutex poison error"))?;
            
            lock.execute(
                "INSERT OR REPLACE INTO process_control_blocks (pid, state, data, updated_at) 
                 VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
                (
                    &pcb_clone.pid,
                    format!("{:?}", pcb_clone.state),
                    json_data,
                ),
            ).context("Failed to execute INSERT/REPLACE on PCB table")?;
            
            debug!(pid = %pcb_clone.pid, "PCB persisted successfully.");
            Ok(())
        })
        .await
        .context("Spawn blocking failed")?
    }

    async fn delete_pcb(&self, pid: &str) -> Result<()> {
        let pid_str = pid.to_string();
        let conn = self.conn.clone();

        task::spawn_blocking(move || {
            let lock = conn.lock().map_err(|_| anyhow::anyhow!("Mutex poison error"))?;
            lock.execute(
                "DELETE FROM process_control_blocks WHERE pid = ?1",
                [&pid_str],
            ).context("Failed to delete PCB from disk")?;
            Ok(())
        })
        .await
        .context("Spawn blocking failed")?
    }

    async fn load_all_pcbs(&self) -> Result<Vec<PCB>> {
        let conn = self.conn.clone();

        task::spawn_blocking(move || {
            let lock = conn.lock().map_err(|_| anyhow::anyhow!("Mutex poison error"))?;
            let mut stmt = lock.prepare("SELECT data FROM process_control_blocks")?;
            let pcb_iter = stmt.query_map([], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?;

            let mut results = Vec::new();
            for pcb_json_res in pcb_iter {
                let json_str = pcb_json_res?;
                let pcb: PCB = serde_json::from_str(&json_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
                })?;
                results.push(pcb);
            }
            Ok(results)
        })
        .await
        .context("Spawn blocking failed")?
    }

    async fn flush(&self) -> Result<()> {
        let conn = self.conn.clone();
        task::spawn_blocking(move || {
            let lock = conn.lock().map_err(|_| anyhow::anyhow!("Mutex poison error"))?;
            lock.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])?;
            info!("Persistence flush completed.");
            Ok(())
        })
        .await
        .context("Spawn blocking failed")?
    }
}

#[cfg(test)]
pub struct MockPersistor;

#[cfg(test)]
#[async_trait]
impl StatePersistor for MockPersistor {
    async fn save_pcb(&self, _pcb: &PCB) -> Result<()> { Ok(()) }
    async fn delete_pcb(&self, _pid: &str) -> Result<()> { Ok(()) }
    async fn load_all_pcbs(&self) -> Result<Vec<PCB>> { Ok(Vec::new()) }
    async fn flush(&self) -> Result<()> { Ok(()) }
}
