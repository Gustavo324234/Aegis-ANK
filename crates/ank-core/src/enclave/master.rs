use anyhow::{Context, Result};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Master Admin Enclave para gestionar superadministradores y mapeos de Tenant_ID a Puertos.
/// Se persiste de manera segura con SQLCipher.
#[derive(Clone)]
pub struct MasterEnclave {
    // Usamos Arc<Mutex<Connection>> para permitir que múltiples hilos o tareas de Tokio
    // compartan de forma segura la misma conexión bloqueante subyacente de libsqlite3.
    // El Mutex se bloquea por períodos muy cortos sólo durante la ejecución de las sentencias,
    // garantizando acceso exclusivo por tarea y previniendo Race Conditions y Deadlocks.
    connection: Arc<Mutex<Connection>>,
}

impl MasterEnclave {
    /// Inicializa o abre la base de datos maestra (admin.db) en el root
    pub async fn open(db_path: &str, master_key: &str) -> Result<Self> {
        let path = Path::new(db_path);
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory for admin db"))?;
            }
        }

        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open master database at {}", db_path))?;

        // Aplicamos la llave. El sistema Aegis pasará una llave estática configurada en variables de entorno,
        // o generada en runtime para encriptar la propia BD maestra si se desea.
        conn.pragma_update(None, "key", master_key)
            .context("Failed to apply PRAGMA key to master database")?;

        // Verificación básica de integridad y capacidad de desencriptación.
        // Si el PRAGMA key falló o la DB está corrupta, esta consulta fallará.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .context("Decryption failed: invalid master key or corrupted master database file")?;

        info!("Master Admin Enclave initialized successfully.");

        let enclave = Self { connection: Arc::new(Mutex::new(conn)) };
        enclave.init_schema().await?;

        Ok(enclave)
    }

    async fn init_schema(&self) -> Result<()> {
        let conn = self.connection.lock().await;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS master_admin (
                id INTEGER PRIMARY KEY DEFAULT 1,
                username TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        ).context("Failed to init master_admin table")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS tenants (
                tenant_id TEXT PRIMARY KEY,
                network_port INTEGER NOT NULL,
                password_must_change INTEGER NOT NULL DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        ).context("Failed to init tenants table")?;

        Ok(())
    }

    /// Hashea una clave usando SHA-256
    pub fn hash_password(password: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verifica si ya existe un master admin configurado de forma robusta.
    /// Devuelve false si la tabla no existe o si no hay registros.
    pub async fn is_initialized(&self) -> Result<bool> {
        let conn = self.connection.lock().await;

        // Primero verificamos que la tabla exista consultando sqlite_master.
        // Si no existe (ej: DB acaba de ser creada pero init_schema no terminó), es false.
        let table_exists: bool = conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='master_admin'",
            [],
            |_| Ok(true)
        ).unwrap_or(false);

        if !table_exists {
            return Ok(false);
        }

        let count: i64 = conn.query_row("SELECT count(*) FROM master_admin", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    /// Inicializa el super administrador (solo si no hay ninguno)
    pub async fn initialize_master(&self, username: &str, passphrase: &str) -> Result<()> {
        if self.is_initialized().await? {
            anyhow::bail!("Master Admin is already initialized. Cannot overwrite.");
        }

        let hash = Self::hash_password(passphrase);
        let conn = self.connection.lock().await;
        conn.execute(
            "INSERT INTO master_admin (id, username, password_hash) VALUES (1, ?1, ?2)",
            [&username, &hash.as_str()],
        ).context("Failed to configure Master Admin")?;

        info!("Master admin {} successfully configured.", username);
        Ok(())
    }

    /// Valida que el session_key proporcione matching real con el Master Admin password.
    /// Es vital validar tanto username como password_hash para identidad robusta.
    pub async fn authenticate_master(&self, username: &str, passphrase_or_session: &str) -> Result<bool> {
        let conn = self.connection.lock().await;
        
        // Buscamos el hash del admin específico por su username
        let mut stmt = conn.prepare("SELECT password_hash FROM master_admin WHERE username = ?1 LIMIT 1")?;
        
        let hash_result: rusqlite::Result<String> = stmt.query_row([username], |row| row.get(0));
        
        match hash_result {
            Ok(real_hash) => {
                let input_hash = Self::hash_password(passphrase_or_session);
                // Comparamos el hash de la entrada con el persistido
                Ok(input_hash == real_hash)
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false), // Admin no encontrado
            Err(e) => Err(anyhow::anyhow!("Database authentication error: {}", e)),
        }
    }

    /// Genera un nuevo tenant con puerto incrementado asignado, y lo registra
    pub async fn create_tenant(&self, tenant_id: &str) -> Result<(u32, String)> {
        let conn = self.connection.lock().await;
        // En un escenario real, buscaríamos el último puerto usado.
        let mut stmt = conn.prepare("SELECT MAX(network_port) FROM tenants")?;
        let max_port: Option<u32> = stmt.query_row([], |row| row.get(0)).unwrap_or(Some(50051));
        
        // Asignamos el siguiente puerto disponible, empezando desde 50052 para los tenants.
        let next_port = if let Some(p) = max_port {
            if p >= 50052 { p + 1 } else { 50052 }
        } else {
            50052
        };

        // Generar passphrase temporal, e.g., uuid-base o hash. Usaremos uuid simplificado
        let temp_passphrase = uuid::Uuid::new_v4().to_string().replace("-", "")[0..12].to_string();

        conn.execute(
            "INSERT INTO tenants (tenant_id, network_port, password_must_change) VALUES (?1, ?2, 1)",
            rusqlite::params![tenant_id, next_port],
        ).with_context(|| format!("Failed to create tenant {}", tenant_id))?;

        info!("Created tenant {} assigned to port {}", tenant_id, next_port);

        // Devolvemos el puerto y la contraseña temporal sin encriptar, solo para devolvérsela al cliente ahora
        Ok((next_port, temp_passphrase))
    }

    /// Resetea la contraseña forzosamente marcando el flag, pero sin guardar la pass del tenant en master
    pub async fn reset_tenant_password(&self, tenant_id: &str, _new_passphrase: &str) -> Result<()> {
        let conn = self.connection.lock().await;
        let rows = conn.execute(
            "UPDATE tenants SET password_must_change = 1 WHERE tenant_id = ?1",
            [tenant_id],
        ).context("Failed to reset tenant password state")?;

        if rows == 0 {
            anyhow::bail!("Tenant {} not found.", tenant_id);
        }

        info!("Forced password reset for tenant {}", tenant_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_master_admin_flow() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("admin.db");
        let path_str = db_path.to_str().unwrap();

        let enclave = MasterEnclave::open(path_str, "secret_key").await.unwrap();
        
        assert!(!enclave.is_initialized().await.unwrap());
        
        enclave.initialize_master("root", "haxor").await.unwrap();
        assert!(enclave.is_initialized().await.unwrap());
        
        let is_auth = enclave.authenticate_master("root", "haxor").await.unwrap();
        assert!(is_auth);

        let (port, pass) = enclave.create_tenant("testuser").await.unwrap();
        assert!(port >= 50052);
        assert!(!pass.is_empty());

        enclave.reset_tenant_password("testuser", "ignored_for_now").await.unwrap();
    }
}
