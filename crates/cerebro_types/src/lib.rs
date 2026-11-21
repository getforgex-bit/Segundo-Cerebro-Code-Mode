//! Contratos compartidos de Cerebro.
//!
//! Este crate es propiedad del **coordinador**. Cambiarlo requiere una consulta previa
//! (ver `coordination/PROTOCOL.md`). Lo usan:
//! - `cerebro_store`: implementa [`Catalog`], [`AuditLog`] y [`ApprovalStore`].
//! - `cerebro_search`: implementa [`ToolSearch`].
//! - `cerebro_policy`: usa [`ToolRecord`], [`CallCtx`] y devuelve [`Decision`].
//! - `cerebro_hub`: consume [`Catalog`], [`AuditLog`], [`ApprovalStore`] y la política.
//! - `cerebro` (bin): consume todo lo anterior.
//!
//! Regla: aquí solo hay tipos de datos y traits, sin lógica de negocio ni dependencias pesadas.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Result<T> = anyhow::Result<T>;

// ───────────────────────────── Catálogo ─────────────────────────────

/// Servidor MCP downstream tal como lo ve el catálogo (sin comandos ni secretos, que viven en la config).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerRecord {
    /// Nombre estable de la config (p. ej. `github`).
    pub name: String,
    /// Namespace TS que verá la IA (lo calcula pctx; p. ej. `github`).
    pub ts_namespace: String,
    pub status: ServerStatus,
    pub last_error: Option<String>,
    pub protocol_version: Option<String>,
    /// `instructions` que el servidor envía en `initialize`.
    pub instructions: Option<String>,
    /// Hash (blake3 hex) del catálogo completo del servidor.
    pub catalog_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Unknown,
    Ok,
    Error,
    Disabled,
}

/// Herramienta downstream catalogada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRecord {
    /// Nombre del servidor ([`ServerRecord::name`]).
    pub server: String,
    /// Nombre MCP original (p. ej. `create_issue`).
    pub name: String,
    /// Nombre TS que verá la IA (p. ej. `createIssue`), como lo genere pctx.
    pub ts_name: String,
    pub title: Option<String>,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    /// Anotaciones MCP crudas (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`).
    pub annotations: Value,
    /// Firma TS compacta (una línea) para `search_functions`.
    pub ts_signature: Option<String>,
    /// Coste estimado en tokens del esquema crudo (línea base de ahorro).
    pub approx_tokens: u32,
    /// blake3 hex de (name, description, input_schema, output_schema, annotations).
    pub schema_hash: String,
    /// Etiquetas de taint configuradas (M5): `private-data`, `untrusted-input`, `external-write`.
    pub taint_labels: Vec<String>,
}

impl ToolRecord {
    /// Identificador cualificado `server.name`.
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.server, self.name)
    }

    pub fn read_only_hint(&self) -> Option<bool> {
        self.annotations.get("readOnlyHint").and_then(Value::as_bool)
    }

    pub fn destructive_hint(&self) -> Option<bool> {
        self.annotations
            .get("destructiveHint")
            .and_then(Value::as_bool)
    }
}

/// Persistencia del catálogo (la implementa `cerebro_store`).
pub trait Catalog: Send + Sync {
    /// Inserta o actualiza el servidor y **reemplaza** de forma atómica su lista de herramientas.
    fn replace_server_tools(&self, server: &ServerRecord, tools: &[ToolRecord]) -> Result<()>;
    fn update_server_status(
        &self,
        server: &str,
        status: ServerStatus,
        last_error: Option<&str>,
    ) -> Result<()>;
    fn get_tool(&self, server: &str, name: &str) -> Result<Option<ToolRecord>>;
    fn list_tools(&self, server: Option<&str>) -> Result<Vec<ToolRecord>>;
    fn list_servers(&self) -> Result<Vec<ServerRecord>>;
}

// ───────────────────────────── Búsqueda ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Detail {
    /// Solo `server.tool`.
    Names,
    /// Nombre + descripción de una línea + marca `[read]`/`[write]`/`[destructive]`.
    #[default]
    Summary,
    /// Summary + firma TS.
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchOpts {
    pub detail: Detail,
    pub server: Option<String>,
    /// Por defecto 8, máximo 25.
    pub limit: usize,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            detail: Detail::Summary,
            server: None,
            limit: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolHit {
    pub tool: ToolRecord,
    /// Mayor es mejor (RRF de BM25 + trigram).
    pub score: f64,
}

/// Búsqueda sobre el catálogo (la implementa `cerebro_search`).
pub trait ToolSearch: Send + Sync {
    fn search(&self, query: &str, opts: &SearchOpts) -> Result<Vec<ToolHit>>;
}

// ───────────────────────────── Política ─────────────────────────────

/// Contexto de una llamada a herramienta desde una ejecución de `execute_typescript`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CallCtx {
    /// ULID de la ejecución (agrupa llamadas para auditoría y taint).
    pub execution_id: String,
    /// Tokens de aprobación que el cliente envió en `execute_typescript({ approvals })`.
    pub approvals: Vec<String>,
    /// Etiquetas de taint acumuladas en esta ejecución hasta ahora (M5).
    pub taint_seen: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Decision {
    Allow,
    /// Requiere aprobación humana antes de ejecutar.
    Confirm { reason: String },
    Deny { reason: String },
}

// ───────────────────────────── Aprobaciones ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub execution_id: String,
    /// `server.tool`.
    pub tool: String,
    /// blake3 hex de los argumentos canónicos (JSON con claves ordenadas).
    pub args_hash: String,
    pub reason: String,
    /// Segundos de validez (por defecto 600).
    pub ttl_seconds: u32,
}

/// Aprobaciones de un solo uso (las implementa `cerebro_store`).
pub trait ApprovalStore: Send + Sync {
    /// Crea una aprobación pendiente y devuelve su token (opaco, aleatorio).
    fn create(&self, req: &ApprovalRequest) -> Result<String>;
    /// Marca el token como aprobado por el usuario (lo usa el flujo MRTR o la CLI).
    fn approve(&self, token: &str) -> Result<bool>;
    /// Consume el token si existe, no expiró, coincide con `tool` y `args_hash`, y está
    /// pendiente o aprobado. Devuelve `true` si la llamada puede ejecutarse. Un solo uso.
    fn consume(&self, token: &str, tool: &str, args_hash: &str) -> Result<bool>;
}

// ───────────────────────────── Auditoría ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallDecision {
    Allowed,
    Confirmed,
    Denied,
    NeedsApproval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallStatus {
    Ok,
    Error,
    Timeout,
    NotRun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub execution_id: String,
    pub server: String,
    pub tool: String,
    /// Argumentos con secretos redactados (campos tipo token/password/secret/key → "***").
    pub args_redacted: Value,
    pub args_hash: String,
    pub decision: CallDecision,
    pub status: CallStatus,
    pub error: Option<String>,
    pub result_bytes: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub execution_id: String,
    pub code_hash: String,
    pub status: String,
    pub error: Option<String>,
    pub tool_calls: u32,
    /// Bytes de resultados intermedios que NO pasaron por el modelo.
    pub raw_bytes_processed: u64,
    /// Bytes devueltos al modelo.
    pub result_bytes: u64,
    pub duration_ms: u64,
}

/// Resumen de una llamada que el servidor devuelve a la IA dentro de `execute_typescript`
/// (`calls: [{tool, ms, ok}]`). Lo acumula el hub por ejecución.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallSummary {
    /// `server.tool`.
    pub tool: String,
    pub ms: u64,
    pub ok: bool,
    pub decision: CallDecision,
    /// Bytes del resultado que se quedaron en el sandbox (no pasaron por el modelo).
    pub result_bytes: u64,
}

/// Auditoría (la implementa `cerebro_store`).
pub trait AuditLog: Send + Sync {
    fn record_tool_call(&self, rec: &ToolCallRecord) -> Result<()>;
    fn record_execution(&self, rec: &ExecutionRecord) -> Result<()>;
}

// ───────────────────── Errores visibles dentro del sandbox ─────────────────────

/// Código de error estable que ve el código TS de la IA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ToolNotFound,
    InvalidInput,
    NeedsApproval,
    Denied,
    ToolError,
    Timeout,
    LimitExceeded,
    ServerUnavailable,
}

/// Carga útil de error. Los callbacks de pctx devuelven `Err(String)`: el hub **serializa
/// este struct como JSON** en ese `String`, con el prefijo [`ERROR_PREFIX`], para que el
/// servidor y el runtime lo reconozcan sin ambigüedad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CerebroErrorPayload {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_token: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

pub const ERROR_PREFIX: &str = "CEREBRO_ERROR:";

impl CerebroErrorPayload {
    pub fn to_callback_error(&self) -> String {
        format!(
            "{ERROR_PREFIX}{}",
            serde_json::to_string(self).unwrap_or_else(|_| self.message.clone())
        )
    }

    /// Extrae la carga útil de un texto de error (stderr o mensaje de callback), si la contiene.
    pub fn parse_from(text: &str) -> Option<Self> {
        let start = text.find(ERROR_PREFIX)? + ERROR_PREFIX.len();
        let rest = &text[start..];
        let mut de = serde_json::Deserializer::from_str(rest).into_iter::<Self>();
        de.next()?.ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_payload_roundtrip_inside_noise() {
        let p = CerebroErrorPayload {
            code: ErrorCode::NeedsApproval,
            message: "github.deleteRepo requires approval".into(),
            approval_token: Some("tok_123".into()),
            suggestions: vec![],
        };
        let wrapped = format!("Uncaught Error: {} at line 3", p.to_callback_error());
        assert_eq!(CerebroErrorPayload::parse_from(&wrapped), Some(p));
    }

    #[test]
    fn decision_serializes_tagged() {
        let d = Decision::Confirm {
            reason: "destructive".into(),
        };
        assert_eq!(
            serde_json::to_value(&d).unwrap(),
            serde_json::json!({"decision": "confirm", "reason": "destructive"})
        );
    }
}
