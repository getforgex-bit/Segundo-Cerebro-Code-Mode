//! Persistencia SQLite de Cerebro (WS2).
//!
//! - `brain.db` es la base principal (auditoría, aprobaciones, cerebro).
//! - `hub.db` se adjunta como esquema `hub` (catálogo de servidores y herramientas).
//!
//! [`Store`] es `Clone` y comparte una única conexión protegida por un `Mutex`: las
//! llamadas son cortas y síncronas; el hub las envuelve en `spawn_blocking` si hace falta.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, anyhow, bail};
use cerebro_types::{
    ApprovalRequest, ApprovalStore, AuditLog, Catalog, ExecutionRecord, Result, ServerRecord,
    ServerStatus, ToolCallRecord, ToolRecord,
};
use rusqlite::Connection;

/// Rutas de las dos bases de datos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePaths {
    pub brain_db: PathBuf,
    pub hub_db: PathBuf,
}

/// Store SQLite compartido (barato de clonar).
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// Abre (o crea) ambas bases: crea los directorios, aplica pragmas y migraciones y
    /// adjunta `hub.db` como `hub`.
    pub fn open(paths: &StorePaths) -> Result<Self> {
        for p in [&paths.brain_db, &paths.hub_db] {
            if let Some(dir) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("creando el directorio {}", dir.display()))?;
            }
        }
        let conn = Connection::open(&paths.brain_db)
            .with_context(|| format!("abriendo {}", paths.brain_db.display()))?;
        let hub = paths
            .hub_db
            .to_str()
            .ok_or_else(|| anyhow!("ruta de hub.db no es UTF-8"))?
            .to_owned();
        Self::init(conn, &hub, true)
    }

    /// Ambas bases en memoria (para tests).
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?, ":memory:", false)
    }

    fn init(conn: Connection, hub_path: &str, wal: bool) -> Result<Self> {
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.execute("ATTACH DATABASE ?1 AS hub", [hub_path])?;
        if wal {
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(Some("hub"), "journal_mode", "WAL")?;
        }
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Acceso a la conexión (lo usa `cerebro_search`).
    #[doc(hidden)]
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Catalog for Store {
    fn replace_server_tools(&self, _server: &ServerRecord, _tools: &[ToolRecord]) -> Result<()> {
        bail!("cerebro_store: replace_server_tools aún no implementado (H3)")
    }
    fn update_server_status(
        &self,
        _server: &str,
        _status: ServerStatus,
        _last_error: Option<&str>,
    ) -> Result<()> {
        bail!("cerebro_store: update_server_status aún no implementado (H3)")
    }
    fn get_tool(&self, _server: &str, _name: &str) -> Result<Option<ToolRecord>> {
        bail!("cerebro_store: get_tool aún no implementado (H3)")
    }
    fn list_tools(&self, _server: Option<&str>) -> Result<Vec<ToolRecord>> {
        bail!("cerebro_store: list_tools aún no implementado (H3)")
    }
    fn list_servers(&self) -> Result<Vec<ServerRecord>> {
        bail!("cerebro_store: list_servers aún no implementado (H3)")
    }
}

impl AuditLog for Store {
    fn record_tool_call(&self, _rec: &ToolCallRecord) -> Result<()> {
        bail!("cerebro_store: record_tool_call aún no implementado (H3)")
    }
    fn record_execution(&self, _rec: &ExecutionRecord) -> Result<()> {
        bail!("cerebro_store: record_execution aún no implementado (H3)")
    }
}

impl ApprovalStore for Store {
    fn create(&self, _req: &ApprovalRequest) -> Result<String> {
        bail!("cerebro_store: approvals.create aún no implementado (H3)")
    }
    fn approve(&self, _token: &str) -> Result<bool> {
        bail!("cerebro_store: approvals.approve aún no implementado (H3)")
    }
    fn consume(&self, _token: &str, _tool: &str, _args_hash: &str) -> Result<bool> {
        bail!("cerebro_store: approvals.consume aún no implementado (H3)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_in_memory() {
        Store::open_in_memory().unwrap();
    }
}
