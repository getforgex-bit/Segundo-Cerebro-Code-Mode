//! Persistencia SQLite de Cerebro (WS2).
//!
//! - `brain.db` es la base principal (auditoría, aprobaciones, cerebro).
//! - `hub.db` se adjunta como esquema `hub` (catálogo de servidores y herramientas).
//!
//! [`Store`] es `Clone` y comparte una única conexión protegida por un `Mutex`: las
//! llamadas son cortas y síncronas; el hub las envuelve en `spawn_blocking` si hace falta.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, anyhow, bail};
use cerebro_types::{
    ApprovalInfo, ApprovalMode, ApprovalRequest, ApprovalStore, AuditLog, Catalog, ExecutionRecord,
    Result, ServerRecord, ServerStatus, ToolCallRecord, ToolRecord,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use rusqlite_migration::{M, Migrations};
use serde::Serialize;

/// Marca de tiempo RFC 3339 UTC de SQLite (ms), igual que los `DEFAULT` de las tablas.
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

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
    /// En memoria, la BD compartida del hub vive mientras viva alguna conexión: se retiene
    /// la de migración. (`None` en disco.)
    _hub_keepalive: Option<Arc<Mutex<Connection>>>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

fn brain_migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!(
        "../migrations/brain/0001_init.sql"
    ))])
}

fn hub_migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!("../migrations/hub/0001_init.sql"))])
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
        // `user_version` va por archivo: el hub migra por su cuenta, antes del ATTACH.
        {
            let mut hub = Connection::open(&paths.hub_db)
                .with_context(|| format!("abriendo {}", paths.hub_db.display()))?;
            hub.busy_timeout(std::time::Duration::from_millis(5000))?;
            hub.pragma_update(None, "journal_mode", "WAL")?;
            hub.pragma_update(None, "foreign_keys", "ON")?;
            hub_migrations()
                .to_latest(&mut hub)
                .context("migrando hub.db")?;
        }
        let conn = Connection::open(&paths.brain_db)
            .with_context(|| format!("abriendo {}", paths.brain_db.display()))?;
        let hub_path = paths
            .hub_db
            .to_str()
            .ok_or_else(|| anyhow!("ruta de hub.db no es UTF-8"))?;
        Self::init(conn, hub_path, true, None)
    }

    /// Ambas bases en memoria (para tests).
    pub fn open_in_memory() -> Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let uri = format!(
            "file:cerebro-hub-{}-{n}?mode=memory&cache=shared",
            std::process::id()
        );
        let mut hub = Connection::open(&uri).context("abriendo hub en memoria")?;
        hub.pragma_update(None, "foreign_keys", "ON")?;
        hub_migrations()
            .to_latest(&mut hub)
            .context("migrando hub (memoria)")?;
        Self::init(
            Connection::open(format!(
                "file:cerebro-brain-{}-{n}?mode=memory",
                std::process::id()
            ))?,
            &uri,
            false,
            Some(Arc::new(Mutex::new(hub))),
        )
    }

    fn init(
        mut conn: Connection,
        hub_path: &str,
        wal: bool,
        keepalive: Option<Arc<Mutex<Connection>>>,
    ) -> Result<Self> {
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        if wal {
            conn.pragma_update(None, "journal_mode", "WAL")?;
        }
        conn.pragma_update(None, "foreign_keys", "ON")?;
        brain_migrations()
            .to_latest(&mut conn)
            .context("migrando brain.db")?;
        conn.execute("ATTACH DATABASE ?1 AS hub", [hub_path])?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            _hub_keepalive: keepalive,
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

/// Parte un identificador en palabras en minúsculas: separa por caracteres no
/// alfanuméricos y por camelCase (`createIssue` → `create`, `issue`; `HTTPServer` →
/// `http`, `server`; `v2Beta` → `v2`, `beta`).
pub fn split_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in text.split(|c: char| !c.is_alphanumeric()) {
        let chars: Vec<char> = part.chars().collect();
        let mut cur = String::new();
        for (i, &c) in chars.iter().enumerate() {
            if i > 0 && c.is_uppercase() {
                let prev = chars[i - 1];
                let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
                if prev.is_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_uppercase() && next_lower)
                {
                    out.push(std::mem::take(&mut cur));
                }
            }
            cur.extend(c.to_lowercase());
        }
        out.push(cur);
    }
    out.retain(|w| !w.is_empty());
    out
}

fn search_text(server: &str, tool: &ToolRecord) -> String {
    let mut words: Vec<String> = Vec::new();
    for src in [server, &tool.name, &tool.ts_name] {
        words.extend(split_words(src));
    }
    if let Some(t) = &tool.title {
        words.extend(split_words(t));
    }
    let mut seen = std::collections::HashSet::new();
    words.retain(|w| seen.insert(w.clone()));
    words.join(" ")
}

fn enum_str<T: Serialize>(v: &T) -> Result<String> {
    match serde_json::to_value(v)? {
        serde_json::Value::String(s) => Ok(s),
        other => bail!("enum no serializable como texto: {other}"),
    }
}

fn status_from_str(s: &str) -> ServerStatus {
    match s {
        "ok" => ServerStatus::Ok,
        "error" => ServerStatus::Error,
        "disabled" => ServerStatus::Disabled,
        _ => ServerStatus::Unknown,
    }
}

const TOOL_COLS: &str = "server, name, ts_name, title, description, input_schema, output_schema, \
                         annotations, ts_signature, approx_tokens, schema_hash, taint_labels";

fn json_col(row: &Row<'_>, idx: usize) -> rusqlite::Result<serde_json::Value> {
    let s: String = row.get(idx)?;
    serde_json::from_str(&s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
    })
}

/// Fila → [`ToolRecord`] (columnas en el orden de `TOOL_COLS`).
fn tool_from_row(row: &Row<'_>) -> rusqlite::Result<ToolRecord> {
    let output_schema = match row.get::<_, Option<String>>(6)? {
        Some(s) => Some(serde_json::from_str(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
        })?),
        None => None,
    };
    let taint: Vec<String> = serde_json::from_value(json_col(row, 11)?).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(ToolRecord {
        server: row.get(0)?,
        name: row.get(1)?,
        ts_name: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        input_schema: json_col(row, 5)?,
        output_schema,
        annotations: json_col(row, 7)?,
        ts_signature: row.get(8)?,
        approx_tokens: row.get(9)?,
        schema_hash: row.get(10)?,
        taint_labels: taint,
    })
}

#[doc(hidden)]
pub fn tool_columns() -> &'static str {
    TOOL_COLS
}

#[doc(hidden)]
pub fn map_tool_row(row: &Row<'_>) -> rusqlite::Result<ToolRecord> {
    tool_from_row(row)
}

impl Catalog for Store {
    fn replace_server_tools(&self, server: &ServerRecord, tools: &[ToolRecord]) -> Result<()> {
        if let Some(t) = tools.iter().find(|t| t.server != server.name) {
            bail!(
                "la herramienta {} pertenece a «{}», no a «{}»",
                t.name,
                t.server,
                server.name
            );
        }
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            &format!(
                "INSERT INTO hub.servers (name, ts_namespace, status, last_error, protocol_version, \
                 instructions, catalog_hash, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, {NOW}) \
                 ON CONFLICT (name) DO UPDATE SET ts_namespace = excluded.ts_namespace, \
                 status = excluded.status, last_error = excluded.last_error, \
                 protocol_version = excluded.protocol_version, instructions = excluded.instructions, \
                 catalog_hash = excluded.catalog_hash, updated_at = excluded.updated_at"
            ),
            params![
                server.name,
                server.ts_namespace,
                enum_str(&server.status)?,
                server.last_error,
                server.protocol_version,
                server.instructions,
                server.catalog_hash,
            ],
        )?;
        {
            let mut up = tx.prepare(&format!(
                "INSERT INTO hub.tools (server, name, ts_name, title, description, input_schema, \
                 output_schema, annotations, ts_signature, search_text, approx_tokens, schema_hash, \
                 taint_labels, available, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 1, {NOW}) \
                 ON CONFLICT (server, name) DO UPDATE SET ts_name = excluded.ts_name, \
                 title = excluded.title, description = excluded.description, \
                 input_schema = excluded.input_schema, output_schema = excluded.output_schema, \
                 annotations = excluded.annotations, ts_signature = excluded.ts_signature, \
                 search_text = excluded.search_text, approx_tokens = excluded.approx_tokens, \
                 schema_hash = excluded.schema_hash, taint_labels = excluded.taint_labels, \
                 available = 1, updated_at = excluded.updated_at"
            ))?;
            for t in tools {
                up.execute(params![
                    t.server,
                    t.name,
                    t.ts_name,
                    t.title,
                    t.description,
                    serde_json::to_string(&t.input_schema)?,
                    t.output_schema
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()?,
                    serde_json::to_string(&t.annotations)?,
                    t.ts_signature,
                    search_text(&server.name, t),
                    t.approx_tokens,
                    t.schema_hash,
                    serde_json::to_string(&t.taint_labels)?,
                ])?;
            }
        }
        // Borra las que ya no vienen (los triggers limpian las FTS).
        let keep: std::collections::HashSet<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        let existing: Vec<String> = {
            let mut st = tx.prepare("SELECT name FROM hub.tools WHERE server = ?1")?;
            st.query_map([&server.name], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for name in existing.iter().filter(|n| !keep.contains(n.as_str())) {
            tx.execute(
                "DELETE FROM hub.tools WHERE server = ?1 AND name = ?2",
                params![server.name, name],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn update_server_status(
        &self,
        server: &str,
        status: ServerStatus,
        last_error: Option<&str>,
    ) -> Result<()> {
        self.conn().execute(
            &format!(
                "INSERT INTO hub.servers (name, ts_namespace, status, last_error, updated_at) \
                 VALUES (?1, ?1, ?2, ?3, {NOW}) \
                 ON CONFLICT (name) DO UPDATE SET status = excluded.status, \
                 last_error = excluded.last_error, updated_at = excluded.updated_at"
            ),
            params![server, enum_str(&status)?, last_error],
        )?;
        Ok(())
    }

    fn get_tool(&self, server: &str, name: &str) -> Result<Option<ToolRecord>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {TOOL_COLS} FROM hub.tools WHERE server = ?1 AND name = ?2"),
                params![server, name],
                tool_from_row,
            )
            .optional()?)
    }

    fn list_tools(&self, server: Option<&str>) -> Result<Vec<ToolRecord>> {
        let conn = self.conn();
        let mut st = conn.prepare(&format!(
            "SELECT {TOOL_COLS} FROM hub.tools WHERE (?1 IS NULL OR server = ?1) \
             ORDER BY server, name"
        ))?;
        let rows = st.query_map([server], tool_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn list_servers(&self) -> Result<Vec<ServerRecord>> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT name, ts_namespace, status, last_error, protocol_version, instructions, \
             catalog_hash FROM hub.servers ORDER BY name",
        )?;
        let rows = st.query_map([], |r| {
            Ok(ServerRecord {
                name: r.get(0)?,
                ts_namespace: r.get(1)?,
                status: status_from_str(&r.get::<_, String>(2)?),
                last_error: r.get(3)?,
                protocol_version: r.get(4)?,
                instructions: r.get(5)?,
                catalog_hash: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

impl AuditLog for Store {
    fn record_tool_call(&self, rec: &ToolCallRecord) -> Result<()> {
        self.conn().execute(
            "INSERT INTO tool_calls (execution_id, server, tool, args_redacted, args_hash, \
             decision, status, error, result_bytes, duration_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                rec.execution_id,
                rec.server,
                rec.tool,
                serde_json::to_string(&rec.args_redacted)?,
                rec.args_hash,
                enum_str(&rec.decision)?,
                enum_str(&rec.status)?,
                rec.error,
                i64::try_from(rec.result_bytes).unwrap_or(i64::MAX),
                i64::try_from(rec.duration_ms).unwrap_or(i64::MAX),
            ],
        )?;
        Ok(())
    }

    fn record_execution(&self, rec: &ExecutionRecord) -> Result<()> {
        self.conn().execute(
            "INSERT INTO executions (execution_id, code_hash, status, error, tool_calls, \
             raw_bytes_processed, result_bytes, duration_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT (execution_id) DO UPDATE SET code_hash = excluded.code_hash, \
             status = excluded.status, error = excluded.error, tool_calls = excluded.tool_calls, \
             raw_bytes_processed = excluded.raw_bytes_processed, \
             result_bytes = excluded.result_bytes, duration_ms = excluded.duration_ms",
            params![
                rec.execution_id,
                rec.code_hash,
                rec.status,
                rec.error,
                rec.tool_calls,
                i64::try_from(rec.raw_bytes_processed).unwrap_or(i64::MAX),
                i64::try_from(rec.result_bytes).unwrap_or(i64::MAX),
                i64::try_from(rec.duration_ms).unwrap_or(i64::MAX),
            ],
        )?;
        Ok(())
    }
}

fn random_token() -> Result<String> {
    let mut buf = [0u8; 16]; // 128 bits
    getrandom::fill(&mut buf).map_err(|e| anyhow!("sin entropía del sistema: {e}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

impl ApprovalStore for Store {
    fn create(&self, req: &ApprovalRequest) -> Result<String> {
        let token = random_token()?;
        self.conn().execute(
            "INSERT INTO approvals (token, execution_id, tool, args_hash, reason, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?6 || ' seconds'))",
            params![
                token,
                req.execution_id,
                req.tool,
                req.args_hash,
                req.reason,
                req.ttl_seconds
            ],
        )?;
        Ok(token)
    }

    fn approve(&self, token: &str) -> Result<bool> {
        let n = self.conn().execute(
            &format!(
                "UPDATE approvals SET status = 'approved', approved_at = {NOW} \
                 WHERE token = ?1 AND status = 'pending' AND expires_at > {NOW}"
            ),
            [token],
        )?;
        Ok(n == 1)
    }

    fn consume(
        &self,
        token: &str,
        tool: &str,
        args_hash: &str,
        mode: ApprovalMode,
    ) -> Result<bool> {
        let allowed = match mode {
            ApprovalMode::OutOfBand => "('approved')",
            ApprovalMode::ClientConfirm => "('pending', 'approved')",
        };
        let n = self.conn().execute(
            &format!(
                "UPDATE approvals SET status = 'consumed', consumed_at = {NOW} \
                 WHERE token = ?1 AND tool = ?2 AND args_hash = ?3 \
                 AND status IN {allowed} AND expires_at > {NOW}"
            ),
            params![token, tool, args_hash],
        )?;
        Ok(n == 1)
    }

    fn list_pending(&self) -> Result<Vec<ApprovalInfo>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT token, execution_id, tool, reason, created_at, expires_at FROM approvals \
             WHERE status = 'pending' AND expires_at > {NOW} ORDER BY created_at DESC"
        ))?;
        let rows = stmt.query_map([], |r| {
            Ok(ApprovalInfo {
                token: r.get(0)?,
                execution_id: r.get(1)?,
                tool: r.get(2)?,
                reason: r.get(3)?,
                created_at: r.get(4)?,
                expires_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cerebro_types::{CallDecision, CallStatus};
    use serde_json::json;

    fn server(name: &str) -> ServerRecord {
        ServerRecord {
            name: name.into(),
            ts_namespace: name.into(),
            status: ServerStatus::Ok,
            last_error: None,
            protocol_version: Some("2025-06-18".into()),
            instructions: None,
            catalog_hash: Some("h1".into()),
        }
    }

    fn tool(server: &str, name: &str, ts_name: &str, desc: &str) -> ToolRecord {
        ToolRecord {
            server: server.into(),
            name: name.into(),
            ts_name: ts_name.into(),
            title: None,
            description: desc.into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations: json!({"readOnlyHint": true}),
            ts_signature: Some(format!("{ts_name}(input: {{}}): Promise<unknown>")),
            approx_tokens: 42,
            schema_hash: format!("hash-{name}"),
            taint_labels: vec![],
        }
    }

    fn fts_rows(store: &Store, table: &str, q: &str) -> Vec<i64> {
        let conn = store.conn();
        let mut st = conn
            .prepare(&format!(
                "SELECT rowid FROM hub.{table} WHERE {table} MATCH ?1 ORDER BY rowid"
            ))
            .unwrap();
        st.query_map([q], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn temp_paths(tag: &str) -> (std::path::PathBuf, StorePaths) {
        let dir = std::env::temp_dir().join(format!(
            "cerebro-store-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = StorePaths {
            brain_db: dir.join("nested/brain.db"),
            hub_db: dir.join("nested/hub.db"),
        };
        (dir, paths)
    }

    #[test]
    fn opens_in_memory_and_instances_are_isolated() {
        let a = Store::open_in_memory().unwrap();
        let b = Store::open_in_memory().unwrap();
        a.update_server_status("x", ServerStatus::Ok, None).unwrap();
        assert_eq!(a.list_servers().unwrap().len(), 1);
        assert!(b.list_servers().unwrap().is_empty());
    }

    #[test]
    fn disk_open_is_idempotent_and_persists() {
        let (dir, paths) = temp_paths("reopen");
        {
            let s = Store::open(&paths).unwrap();
            s.replace_server_tools(&server("gh"), &[tool("gh", "a_b", "aB", "d")])
                .unwrap();
            let mode: String = s
                .conn()
                .query_row("PRAGMA journal_mode", [], |r| r.get(0))
                .unwrap();
            assert_eq!(mode, "wal");
        }
        let s = Store::open(&paths).unwrap();
        assert_eq!(s.list_tools(None).unwrap().len(), 1);
        let (bv, hv): (i64, i64) = {
            let c = s.conn();
            (
                c.query_row("PRAGMA main.user_version", [], |r| r.get(0))
                    .unwrap(),
                c.query_row("PRAGMA hub.user_version", [], |r| r.get(0))
                    .unwrap(),
            )
        };
        assert_eq!((bv, hv), (1, 1));
        let fk: i64 = s
            .conn()
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn split_words_handles_snake_camel_acronyms() {
        assert_eq!(split_words("create_issue"), ["create", "issue"]);
        assert_eq!(split_words("createIssue"), ["create", "issue"]);
        assert_eq!(
            split_words("HTTPServer-v2Beta"),
            ["http", "server", "v2", "beta"]
        );
        assert!(split_words("  --  ").is_empty());
    }

    #[test]
    fn replace_server_tools_upserts_keeps_ids_and_deletes_missing() {
        let s = Store::open_in_memory().unwrap();
        let srv = server("github");
        s.replace_server_tools(
            &srv,
            &[
                tool("github", "create_issue", "createIssue", "Create an issue"),
                tool("github", "list_issues", "listIssues", "List issues"),
            ],
        )
        .unwrap();
        let id_of = |n: &str| -> i64 {
            s.conn()
                .query_row("SELECT id FROM hub.tools WHERE name = ?1", [n], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        let id_create = id_of("create_issue");
        assert_eq!(fts_rows(&s, "tools_fts", "\"create\""), vec![id_create]);

        let mut changed = tool("github", "create_issue", "createIssue", "Open a ticket");
        changed.title = Some("Nuevo título".into());
        s.replace_server_tools(&srv, &[changed]).unwrap();
        assert_eq!(id_of("create_issue"), id_create, "el id se conserva");
        assert_eq!(s.list_tools(Some("github")).unwrap().len(), 1);
        assert!(s.get_tool("github", "list_issues").unwrap().is_none());
        // FTS: el update queda sincronizado y la herramienta borrada ya no está
        assert!(fts_rows(&s, "tools_fts", "\"list\"").is_empty());
        assert_eq!(fts_rows(&s, "tools_fts", "\"ticket\""), vec![id_create]);
        assert_eq!(fts_rows(&s, "tools_trgm", "\"ick\""), vec![id_create]);
        let t = s.get_tool("github", "create_issue").unwrap().unwrap();
        assert_eq!(t.title.as_deref(), Some("Nuevo título"));
        assert_eq!(t.annotations, json!({"readOnlyHint": true}));
    }

    #[test]
    fn replace_rejects_foreign_tools_and_is_atomic() {
        let s = Store::open_in_memory().unwrap();
        s.replace_server_tools(&server("a"), &[tool("a", "x", "x", "one")])
            .unwrap();
        let bad = [tool("a", "y", "y", "two"), tool("b", "z", "z", "three")];
        assert!(s.replace_server_tools(&server("a"), &bad).is_err());
        let names: Vec<_> = s
            .list_tools(None)
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, ["x"]);
    }

    #[test]
    fn deleting_a_server_cascades_to_tools_and_fts() {
        let s = Store::open_in_memory().unwrap();
        s.replace_server_tools(
            &server("gh"),
            &[tool("gh", "create_issue", "createIssue", "Create")],
        )
        .unwrap();
        s.replace_server_tools(
            &server("fs"),
            &[tool("fs", "read_file", "readFile", "Read")],
        )
        .unwrap();
        s.conn()
            .execute(
                "INSERT INTO hub.tool_stats (server, name, calls) VALUES ('gh', 'create_issue', 3)",
                [],
            )
            .unwrap();
        assert_eq!(fts_rows(&s, "tools_fts", "\"create\"").len(), 1);
        s.conn()
            .execute("DELETE FROM hub.servers WHERE name = 'gh'", [])
            .unwrap();
        assert!(s.list_tools(Some("gh")).unwrap().is_empty());
        assert!(fts_rows(&s, "tools_fts", "\"create\"").is_empty());
        assert!(fts_rows(&s, "tools_trgm", "\"reate\"").is_empty());
        assert_eq!(fts_rows(&s, "tools_fts", "\"read\"").len(), 1);
        let stats: i64 = s
            .conn()
            .query_row("SELECT count(*) FROM hub.tool_stats", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stats, 0);
        // la FTS external content sigue íntegra
        s.conn()
            .execute(
                "INSERT INTO hub.tools_fts (tools_fts) VALUES ('integrity-check')",
                [],
            )
            .unwrap();
    }

    #[test]
    fn update_server_status_upserts() {
        let s = Store::open_in_memory().unwrap();
        s.update_server_status("new", ServerStatus::Error, Some("boom"))
            .unwrap();
        let srv = &s.list_servers().unwrap()[0];
        assert_eq!(
            (srv.status, srv.last_error.as_deref()),
            (ServerStatus::Error, Some("boom"))
        );
        assert_eq!(srv.ts_namespace, "new");
        s.update_server_status("new", ServerStatus::Ok, None)
            .unwrap();
        let srv = &s.list_servers().unwrap()[0];
        assert_eq!(
            (srv.status, srv.last_error.clone()),
            (ServerStatus::Ok, None)
        );
    }

    #[test]
    fn audit_records_calls_before_execution_and_upserts_execution() {
        let s = Store::open_in_memory().unwrap();
        s.record_tool_call(&ToolCallRecord {
            execution_id: "exec1".into(),
            server: "echo".into(),
            tool: "echo".into(),
            args_redacted: json!({"token": "***"}),
            args_hash: "h".into(),
            decision: CallDecision::Allowed,
            status: CallStatus::Ok,
            error: None,
            result_bytes: 10,
            duration_ms: 3,
        })
        .unwrap();
        let mut ex = ExecutionRecord {
            execution_id: "exec1".into(),
            code_hash: "c".into(),
            status: "running".into(),
            error: None,
            tool_calls: 1,
            raw_bytes_processed: 4000,
            result_bytes: 400,
            duration_ms: 5,
        };
        s.record_execution(&ex).unwrap();
        ex.status = "ok".into();
        s.record_execution(&ex).unwrap();
        let c = s.conn();
        let n: i64 = c
            .query_row("SELECT count(*) FROM executions", [], |r| r.get(0))
            .unwrap();
        let (st, saved): (String, i64) = c
            .query_row(
                "SELECT e.status, v.saved_bytes FROM v_token_savings v \
                 JOIN executions e USING (execution_id)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((n, st.as_str(), saved), (1, "ok", 3600));
        let calls: i64 = c
            .query_row("SELECT count(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(calls, 1);
    }

    fn req(ttl: u32) -> ApprovalRequest {
        ApprovalRequest {
            execution_id: "e".into(),
            tool: "gh.delete_repo".into(),
            args_hash: "abc".into(),
            reason: "destructive".into(),
            ttl_seconds: ttl,
        }
    }

    const OOB: ApprovalMode = ApprovalMode::OutOfBand;
    const CC: ApprovalMode = ApprovalMode::ClientConfirm;

    #[test]
    fn approval_valid_is_single_use() {
        let s = Store::open_in_memory().unwrap();
        let tok = s.create(&req(600)).unwrap();
        assert_eq!(tok.len(), 32);
        assert!(tok.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(tok, s.create(&req(600)).unwrap());
        assert!(s.approve(&tok).unwrap());
        assert!(s.consume(&tok, "gh.delete_repo", "abc", OOB).unwrap());
        assert!(
            !s.consume(&tok, "gh.delete_repo", "abc", OOB).unwrap(),
            "reutilizado"
        );
        assert!(!s.approve(&tok).unwrap());
    }

    #[test]
    fn approval_pending_requires_human_in_out_of_band_mode() {
        // Seguridad: la IA ve el token en NEEDS_APPROVAL; reenviarlo sin aprobación humana
        // no debe bastar en el modo por defecto.
        let s = Store::open_in_memory().unwrap();
        let tok = s.create(&req(600)).unwrap();
        assert!(!s.consume(&tok, "gh.delete_repo", "abc", OOB).unwrap());
        assert_eq!(
            s.list_pending().unwrap().len(),
            1,
            "sigue pendiente, no se quemó"
        );
        assert!(s.approve(&tok).unwrap());
        assert!(s.consume(&tok, "gh.delete_repo", "abc", OOB).unwrap());
        assert!(s.list_pending().unwrap().is_empty());
    }

    #[test]
    fn approval_consumable_from_pending_only_in_client_confirm_mode() {
        let s = Store::open_in_memory().unwrap();
        let tok = s.create(&req(600)).unwrap();
        assert!(s.consume(&tok, "gh.delete_repo", "abc", CC).unwrap());
    }

    #[test]
    fn approval_expired_is_rejected() {
        let s = Store::open_in_memory().unwrap();
        let tok = s.create(&req(0)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(!s.consume(&tok, "gh.delete_repo", "abc", CC).unwrap());
        assert!(!s.approve(&tok).unwrap());
        assert!(s.list_pending().unwrap().is_empty(), "expirada no se lista");
    }

    #[test]
    fn approval_wrong_tool_or_hash_does_not_burn_token() {
        let s = Store::open_in_memory().unwrap();
        let tok = s.create(&req(600)).unwrap();
        assert!(s.approve(&tok).unwrap());
        assert!(!s.consume(&tok, "gh.other", "abc", OOB).unwrap());
        assert!(!s.consume(&tok, "gh.delete_repo", "zzz", OOB).unwrap());
        assert!(
            !s.consume("no-existe", "gh.delete_repo", "abc", OOB)
                .unwrap()
        );
        assert!(s.consume(&tok, "gh.delete_repo", "abc", OOB).unwrap());
    }

    #[test]
    fn list_pending_shows_tool_and_reason() {
        let s = Store::open_in_memory().unwrap();
        let tok = s.create(&req(600)).unwrap();
        let p = s.list_pending().unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].token, tok);
        assert_eq!(p[0].tool, "gh.delete_repo");
    }

    #[test]
    fn brain_schema_constraints() {
        let s = Store::open_in_memory().unwrap();
        let c = s.conn();
        assert!(
            c.execute(
                "INSERT INTO memories (uid, kind, title) VALUES ('u1', 'bogus', 't')",
                []
            )
            .is_err()
        );
        c.execute(
            "INSERT INTO memories (uid, kind, title, body) \
             VALUES ('u1', 'decision', 'Usar SQLite', 'Elegimos búsqueda léxica')",
            [],
        )
        .unwrap();
        let vis: String = c
            .query_row("SELECT visibility FROM memories", [], |r| r.get(0))
            .unwrap();
        assert_eq!(vis, "team");
        // sin porter, con diacríticos normalizados y prefijo
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'busq*'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        // chunks.symbol_id queda NULL al borrar el símbolo; borrar el documento limpia la FTS
        c.execute(
            "INSERT INTO documents (path, kind, hash) VALUES ('a.rs', 'code', 'h')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO symbols (document_id, name, kind) VALUES (1, 'main', 'function')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO chunks (document_id, symbol_id, content) VALUES (1, 1, 'fn main')",
            [],
        )
        .unwrap();
        c.execute("DELETE FROM symbols", []).unwrap();
        let sid: Option<i64> = c
            .query_row("SELECT symbol_id FROM chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sid, None);
        c.execute("DELETE FROM documents", []).unwrap();
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH 'main'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
        c.execute("INSERT INTO skills (name) VALUES ('s')", [])
            .unwrap();
        let t: i64 = c
            .query_row("SELECT trusted FROM skills", [], |r| r.get(0))
            .unwrap();
        assert_eq!(t, 0);
    }
}
