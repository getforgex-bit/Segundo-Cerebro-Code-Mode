//! Hub downstream de Cerebro (WS3, H3).
//!
//! Toda llamada a un servidor MCP downstream pasa por aquí: política → aprobación →
//! llamada (rmcp 3.11, stdio) → auditoría. Cada herramienta del catálogo se registra en
//! pctx como **callback** (nunca con `add_mcp`), de modo que el sandbox TS no puede
//! saltarse el hub.
//!
//! # Decisiones de diseño
//! - **Una conexión por servidor** (`Conn`), con arranque perezoso y un `tokio::Mutex` que
//!   evita el doble spawn. Las llamadas concurrentes comparten el `Peer` (rmcp multiplexa).
//! - **Runtime**: los callbacks de pctx corren en un runtime `current_thread` efímero. Por
//!   eso `Hub::new` captura `Handle::current()` y cada callback hace
//!   `handle.spawn(hub.call(..)).await`; ningún proceso hijo ni tarea de rmcp nace en el
//!   runtime efímero.
//! - **Nombres TS**: `ts_namespace`/`ts_name` salen de `pctx_code_mode::codegen::case::Case`,
//!   las mismas funciones que usa pctx, para coincidir con lo que ve la IA.
//! - **Colisiones**: dos nombres crudos con el mismo nombre TS se deduplican (gana el
//!   primero por orden alfabético); el resto no entra al catálogo y se avisa en
//!   [`SyncReport::warnings`]. `ToolRecord` no tiene campo `available`, así que "no
//!   disponible" = "no catalogado".

#![allow(clippy::missing_errors_doc)]

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, Weak};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cerebro_policy::Policy;
use cerebro_types::{
    ApprovalMode, ApprovalRequest, ApprovalStore, AuditLog, CallCtx, CallDecision, CallStatus,
    CallSummary, Catalog, CerebroErrorPayload, Decision, ErrorCode, ServerRecord, ServerStatus,
    ToolCallRecord, ToolRecord,
};
use pctx_code_mode::codegen::case::Case;
use pctx_code_mode::model::CallbackConfig;
use pctx_code_mode::registry::PctxRegistry;
use rmcp::model::{CallToolRequestParams, CallToolResponse};
use rmcp::service::{Peer, RunningService, ServiceError};
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::runtime::Handle;
use tokio::sync::Mutex;

/// Tiempo máximo para arrancar un servidor y completar `initialize`.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// TTL de las aprobaciones pendientes.
const APPROVAL_TTL_SECONDS: u32 = 600;

// ───────────────────────────── Configuración ─────────────────────────────

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct DownstreamServer {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct HubConfig {
    pub servers: Vec<DownstreamServer>,
    /// Plazo de cada `tools/call`.
    pub call_timeout: Duration,
    /// Un servidor sin uso durante este tiempo se apaga (se relanza al siguiente uso).
    /// `Duration::ZERO` desactiva el apagado por inactividad.
    pub idle_shutdown: Duration,
    /// `OutOfBand` (por defecto): solo valen tokens aprobados por un humano fuera de banda.
    /// `ClientConfirm`: también tokens pendientes (más débil; opt-in).
    pub approval_mode: ApprovalMode,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            servers: vec![],
            call_timeout: Duration::from_secs(30),
            idle_shutdown: Duration::from_secs(300),
            approval_mode: ApprovalMode::OutOfBand,
        }
    }
}

/// Resultado de [`Hub::sync_catalog`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub servers_ok: Vec<String>,
    pub servers_failed: Vec<(String, String)>,
    pub tools: usize,
    /// Avisos no fatales (colisiones de nombre TS, herramientas descartadas).
    pub warnings: Vec<String>,
}

// ───────────────────────────── Ámbito de ejecución ─────────────────────────────

/// Acumula las llamadas de **una** ejecución de `execute_typescript`.
#[derive(Clone)]
pub struct ExecutionScope {
    pub execution_id: String,
    calls: Arc<StdMutex<Vec<CallSummary>>>,
}

impl ExecutionScope {
    fn new(execution_id: &str) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            calls: Arc::default(),
        }
    }

    fn push(&self, c: CallSummary) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(c);
    }

    pub fn calls(&self) -> Vec<CallSummary> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Bytes de resultados que se quedaron en el sandbox (no pasaron por el modelo).
    pub fn raw_bytes(&self) -> u64 {
        self.calls().iter().map(|c| c.result_bytes).sum()
    }
}

// ───────────────────────────── Conexiones ─────────────────────────────

type Service = RunningService<RoleClient, ()>;

struct Live {
    service: Service,
    peer: Peer<RoleClient>,
}

struct Conn {
    spec: DownstreamServer,
    live: Mutex<Option<Live>>,
    inflight: AtomicUsize,
    /// Milisegundos desde `Hub::epoch` del último uso.
    last_used_ms: AtomicU64,
}

pub struct Hub {
    cfg: HubConfig,
    catalog: Arc<dyn Catalog>,
    audit: Arc<dyn AuditLog>,
    approvals: Arc<dyn ApprovalStore>,
    policy: Policy,
    handle: Handle,
    conns: HashMap<String, Arc<Conn>>,
    /// Servidores rechazados por la config (nombre duplicado o namespace TS repetido).
    config_errors: Vec<(String, String)>,
    epoch: Instant,
    reaper: StdMutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Hub {
    /// Debe llamarse **dentro del runtime tokio principal**: guarda su `Handle`.
    pub fn new(
        cfg: HubConfig,
        catalog: Arc<dyn Catalog>,
        audit: Arc<dyn AuditLog>,
        approvals: Arc<dyn ApprovalStore>,
        policy: Policy,
    ) -> Arc<Self> {
        let mut conns = HashMap::new();
        let mut namespaces: HashMap<String, String> = HashMap::new();
        let mut config_errors = Vec::new();
        for s in &cfg.servers {
            let ns = Case::Pascal.sanitize(&s.name);
            if s.name.is_empty() || ns.is_empty() {
                config_errors.push((s.name.clone(), "invalid server name".to_string()));
            } else if conns.contains_key(&s.name) {
                config_errors.push((s.name.clone(), "duplicate server name".to_string()));
            } else if let Some(other) = namespaces.get(&ns) {
                config_errors.push((
                    s.name.clone(),
                    format!("TS namespace `{ns}` already used by server `{other}`"),
                ));
            } else {
                namespaces.insert(ns, s.name.clone());
                conns.insert(
                    s.name.clone(),
                    Arc::new(Conn {
                        spec: s.clone(),
                        live: Mutex::new(None),
                        inflight: AtomicUsize::new(0),
                        last_used_ms: AtomicU64::new(0),
                    }),
                );
            }
        }
        let hub = Arc::new(Self {
            cfg,
            catalog,
            audit,
            approvals,
            policy,
            handle: Handle::current(),
            conns,
            config_errors,
            epoch: Instant::now(),
            reaper: StdMutex::new(None),
        });
        if !hub.cfg.idle_shutdown.is_zero() {
            let weak = Arc::downgrade(&hub);
            let period = (hub.cfg.idle_shutdown / 2).max(Duration::from_millis(50));
            let jh = hub.handle.spawn(reaper_loop(weak, period));
            *hub.reaper
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(jh);
        }
        hub
    }

    // ───────────── Sincronización del catálogo ─────────────

    /// Arranca cada servidor, hace `tools/list` y reemplaza su catálogo. Un servidor
    /// caído queda con `status=Error` y no afecta a los demás.
    pub async fn sync_catalog(self: &Arc<Self>) -> Result<SyncReport> {
        let mut report = SyncReport::default();
        for (name, why) in &self.config_errors {
            report.servers_failed.push((name.clone(), why.clone()));
        }
        let mut names: Vec<&String> = self.conns.keys().collect();
        names.sort();
        for name in names {
            match self.sync_server(name, &mut report.warnings).await {
                Ok(n) => {
                    report.servers_ok.push(name.clone());
                    report.tools += n;
                }
                Err(e) => {
                    let msg = format!("{e:#}");
                    tracing::warn!(server = %name, error = %msg, "sync failed");
                    self.mark_server_error(name, &msg);
                    report.servers_failed.push((name.clone(), msg));
                }
            }
        }
        Ok(report)
    }

    async fn sync_server(&self, name: &str, warnings: &mut Vec<String>) -> Result<usize> {
        let conn = self.conns.get(name).context("unknown server")?;
        let peer = self.ensure_live(conn).await?;
        let tools = tokio::time::timeout(CONNECT_TIMEOUT, peer.list_all_tools())
            .await
            .context("tools/list timed out")?
            .context("tools/list failed")?;
        let info = peer.peer_info();

        let ts_namespace = Case::Pascal.sanitize(name);
        let mut raw: Vec<(String, &rmcp::model::Tool)> =
            tools.iter().map(|t| (t.name.to_string(), t)).collect();
        raw.sort_by(|a, b| a.0.cmp(&b.0));

        let mut seen: HashMap<String, String> = HashMap::new();
        let mut records = Vec::new();
        for (raw_name, t) in raw {
            let ts_name = Case::Camel.sanitize(&raw_name);
            if ts_name.is_empty() {
                warnings.push(format!("{name}.{raw_name}: empty TS name, skipped"));
                continue;
            }
            if let Some(first) = seen.get(&ts_name) {
                warnings.push(format!(
                    "{name}.{raw_name}: TS name `{ts_namespace}.{ts_name}` collides with \
                     `{first}`; marked unavailable"
                ));
                continue;
            }
            seen.insert(ts_name.clone(), raw_name.clone());
            records.push(tool_record(name, &ts_namespace, &ts_name, t));
        }

        let mut hashes: Vec<&str> = records.iter().map(|r| r.schema_hash.as_str()).collect();
        hashes.sort_unstable();
        let catalog_hash = blake3::hash(hashes.join(",").as_bytes())
            .to_hex()
            .to_string();
        let server = ServerRecord {
            name: name.to_string(),
            ts_namespace,
            status: ServerStatus::Ok,
            last_error: None,
            protocol_version: info.as_ref().map(|i| i.protocol_version.to_string()),
            instructions: info.as_ref().and_then(|i| i.instructions.clone()),
            catalog_hash: Some(catalog_hash),
        };
        self.catalog.replace_server_tools(&server, &records)?;
        Ok(records.len())
    }

    fn mark_server_error(&self, name: &str, msg: &str) {
        let known = self
            .catalog
            .list_servers()
            .is_ok_and(|s| s.iter().any(|r| r.name == name));
        let res = if known {
            self.catalog
                .update_server_status(name, ServerStatus::Error, Some(msg))
        } else {
            // Primer sync fallido: aún no hay fila; se crea sin herramientas.
            let rec = ServerRecord {
                name: name.to_string(),
                ts_namespace: Case::Pascal.sanitize(name),
                status: ServerStatus::Error,
                last_error: Some(msg.to_string()),
                protocol_version: None,
                instructions: None,
                catalog_hash: None,
            };
            self.catalog.replace_server_tools(&rec, &[])
        };
        if let Err(e) = res {
            tracing::error!(server = %name, error = %e, "could not persist server error");
        }
    }

    // ───────────── Conexión perezosa ─────────────

    /// Devuelve el `Peer` del servidor, arrancándolo si hace falta. El mutex evita que dos
    /// llamadas simultáneas lancen el proceso dos veces.
    async fn ensure_live(&self, conn: &Conn) -> Result<Peer<RoleClient>> {
        let mut guard = conn.live.lock().await;
        if let Some(l) = guard.as_ref() {
            if !l.service.is_closed() {
                self.touch(conn);
                return Ok(l.peer.clone());
            }
            tracing::info!(server = %conn.spec.name, "connection closed, restarting");
            *guard = None;
        }
        let live = self.connect(&conn.spec).await?;
        let peer = live.peer.clone();
        *guard = Some(live);
        self.touch(conn);
        Ok(peer)
    }

    async fn connect(&self, spec: &DownstreamServer) -> Result<Live> {
        let mut cmd = tokio::process::Command::new(&spec.command);
        cmd.args(&spec.args).envs(&spec.env);
        if let Some(cwd) = &spec.cwd {
            cmd.current_dir(cwd);
        }
        let (transport, stderr) = TokioChildProcess::builder(cmd)
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("could not spawn `{}`", spec.command))?;
        if let Some(stderr) = stderr {
            let name = spec.name.clone();
            // Debe nacer en el runtime principal (estamos en él: ver docs del módulo).
            self.handle.spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    tracing::debug!(server = %name, "stderr: {l}");
                }
            });
        }
        let service = tokio::time::timeout(CONNECT_TIMEOUT, ().serve(transport))
            .await
            .context("initialize timed out")?
            .context("initialize failed")?;
        let peer = service.peer().clone();
        Ok(Live { service, peer })
    }

    fn touch(&self, conn: &Conn) {
        let ms = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        conn.last_used_ms.store(ms, Ordering::Relaxed);
    }

    async fn reap_idle(&self) {
        let now = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        let idle_ms = u64::try_from(self.cfg.idle_shutdown.as_millis()).unwrap_or(u64::MAX);
        for conn in self.conns.values() {
            if conn.inflight.load(Ordering::Acquire) > 0 {
                continue;
            }
            if now.saturating_sub(conn.last_used_ms.load(Ordering::Relaxed)) < idle_ms {
                continue;
            }
            let mut guard = conn.live.lock().await;
            // Re-comprueba bajo el lock: pudo llegar una llamada entretanto.
            if conn.inflight.load(Ordering::Acquire) > 0 {
                continue;
            }
            if let Some(mut live) = guard.take() {
                tracing::info!(server = %conn.spec.name, "idle shutdown");
                let _ = live.service.close().await;
            }
        }
    }

    /// Cierra todos los procesos hijos (sin zombis) y detiene el reaper.
    pub async fn shutdown(&self) {
        if let Some(jh) = self
            .reaper
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            jh.abort();
        }
        for conn in self.conns.values() {
            let mut guard = conn.live.lock().await;
            if let Some(mut live) = guard.take() {
                let _ = live.service.close().await;
            }
        }
    }

    /// Nº de servidores con proceso vivo (para tests y diagnóstico).
    pub async fn live_servers(&self) -> usize {
        let mut n = 0;
        for conn in self.conns.values() {
            if conn.live.lock().await.is_some() {
                n += 1;
            }
        }
        n
    }

    // ───────────── Llamadas ─────────────

    /// Llamada directa a `server.tool` (la usan los callbacks y los tests).
    pub async fn call(
        self: &Arc<Self>,
        server: &str,
        tool: &str,
        args: Option<Value>,
        ctx: &CallCtx,
    ) -> std::result::Result<Value, CerebroErrorPayload> {
        self.call_inner(server, tool, args, ctx).await.0
    }

    async fn call_inner(
        self: &Arc<Self>,
        server: &str,
        tool: &str,
        args: Option<Value>,
        ctx: &CallCtx,
    ) -> (std::result::Result<Value, CerebroErrorPayload>, CallSummary) {
        let started = Instant::now();
        let qualified = format!("{server}.{tool}");
        let mut summary = CallSummary {
            tool: qualified.clone(),
            ms: 0,
            ok: false,
            decision: CallDecision::Allowed,
            result_bytes: 0,
        };
        let (res, status, decision, args_hash, redacted) =
            self.call_steps(server, tool, args, ctx, &mut summary).await;
        let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        summary.ms = ms;
        summary.decision = decision;
        if let Ok(v) = &res {
            summary.ok = true;
            summary.result_bytes = value_len(v);
        }
        // Una herramienta inexistente no genera registro de auditoría (no hay decisión).
        if let Some(status) = status {
            let rec = ToolCallRecord {
                execution_id: ctx.execution_id.clone(),
                server: server.to_string(),
                tool: tool.to_string(),
                args_redacted: redacted,
                args_hash,
                decision,
                status,
                error: res.as_ref().err().map(|e| e.message.clone()),
                result_bytes: summary.result_bytes,
                duration_ms: ms,
            };
            if let Err(e) = self.audit.record_tool_call(&rec) {
                tracing::error!(tool = %qualified, error = %e, "audit write failed");
            }
        }
        (res, summary)
    }

    /// Cuerpo de `call`. Devuelve también lo necesario para auditar.
    #[allow(clippy::type_complexity)]
    async fn call_steps(
        self: &Arc<Self>,
        server: &str,
        tool: &str,
        args: Option<Value>,
        ctx: &CallCtx,
        _summary: &mut CallSummary,
    ) -> (
        std::result::Result<Value, CerebroErrorPayload>,
        Option<CallStatus>,
        CallDecision,
        String,
        Value,
    ) {
        let qualified = format!("{server}.{tool}");
        let no_audit = |e| {
            (
                Err(e),
                None,
                CallDecision::Denied,
                String::new(),
                Value::Null,
            )
        };

        // 1. Herramienta existente.
        let record = match self.catalog.get_tool(server, tool) {
            Ok(Some(r)) => r,
            Ok(None) => return no_audit(self.not_found(server, tool)),
            Err(e) => {
                return no_audit(payload(
                    ErrorCode::ServerUnavailable,
                    format!("catalog error: {e:#}"),
                ));
            }
        };

        // Los argumentos deben ser un objeto (o ausentes).
        let args_value = args.unwrap_or(Value::Null);
        let args_hash = hash_value(&args_value);
        let redacted = redact(&args_value);
        let arguments: Option<Map<String, Value>> = match &args_value {
            Value::Null => None,
            Value::Object(m) => Some(m.clone()),
            _ => {
                return (
                    Err(payload(
                        ErrorCode::InvalidInput,
                        format!("{qualified}: arguments must be an object"),
                    )),
                    Some(CallStatus::NotRun),
                    CallDecision::Denied,
                    args_hash,
                    redacted,
                );
            }
        };

        // 2. Política y aprobación.
        let mut decision = CallDecision::Allowed;
        match self.policy.decide(&record, ctx) {
            Decision::Allow => {}
            Decision::Deny { reason } => {
                return (
                    Err(payload(ErrorCode::Denied, format!("{qualified}: {reason}"))),
                    Some(CallStatus::NotRun),
                    CallDecision::Denied,
                    args_hash,
                    redacted,
                );
            }
            Decision::Confirm { reason } => {
                let mut approved = false;
                for token in &ctx.approvals {
                    match self.approvals.consume(
                        token,
                        &qualified,
                        &args_hash,
                        self.cfg.approval_mode,
                    ) {
                        Ok(true) => {
                            approved = true;
                            break;
                        }
                        Ok(false) => {}
                        Err(e) => tracing::error!(error = %e, "approval consume failed"),
                    }
                }
                if approved {
                    decision = CallDecision::Confirmed;
                } else {
                    let req = ApprovalRequest {
                        execution_id: ctx.execution_id.clone(),
                        tool: qualified.clone(),
                        args_hash: args_hash.clone(),
                        reason: reason.clone(),
                        ttl_seconds: APPROVAL_TTL_SECONDS,
                    };
                    let err = match self.approvals.create(&req) {
                        Ok(token) => CerebroErrorPayload {
                            code: ErrorCode::NeedsApproval,
                            message: format!(
                                "Approval required: ask the user to run `cerebro approve {token}` \
                                 in an interactive terminal, then retry with approvals: [\"{token}\"]"
                            ),
                            approval_token: Some(token),
                            suggestions: vec![],
                        },
                        Err(e) => payload(
                            ErrorCode::ServerUnavailable,
                            format!("could not create approval: {e:#}"),
                        ),
                    };
                    return (
                        Err(err),
                        Some(CallStatus::NotRun),
                        CallDecision::NeedsApproval,
                        args_hash,
                        redacted,
                    );
                }
            }
        }

        // 3. Arranque perezoso.
        let Some(conn) = self.conns.get(server).cloned() else {
            return no_audit(self.not_found(server, tool));
        };
        conn.inflight.fetch_add(1, Ordering::AcqRel);
        let result = self.run_tool(&conn, tool, arguments).await;
        conn.inflight.fetch_sub(1, Ordering::AcqRel);
        self.touch(&conn);

        let status = match &result {
            Ok(_) => CallStatus::Ok,
            Err(e) if e.code == ErrorCode::Timeout => CallStatus::Timeout,
            Err(_) => CallStatus::Error,
        };
        (result, Some(status), decision, args_hash, redacted)
    }

    async fn run_tool(
        &self,
        conn: &Conn,
        tool: &str,
        arguments: Option<Map<String, Value>>,
    ) -> std::result::Result<Value, CerebroErrorPayload> {
        let qualified = format!("{}.{tool}", conn.spec.name);
        let peer = self.ensure_live(conn).await.map_err(|e| {
            payload(
                ErrorCode::ServerUnavailable,
                format!("{}: cannot start server: {e:#}", conn.spec.name),
            )
        })?;
        let mut params = CallToolRequestParams::new(tool.to_string());
        if let Some(a) = arguments {
            params = params.with_arguments(a);
        }
        let resp = tokio::time::timeout(self.cfg.call_timeout, peer.call_tool_once(params))
            .await
            .map_err(|_| {
                payload(
                    ErrorCode::Timeout,
                    format!(
                        "{qualified} timed out after {} ms",
                        self.cfg.call_timeout.as_millis()
                    ),
                )
            })?;
        let result = match resp {
            Ok(CallToolResponse::Complete(r)) => r,
            Ok(CallToolResponse::InputRequired(_)) => {
                // MRTR queda fuera de la Fase 0 (ADR 0005).
                return Err(payload(
                    ErrorCode::ToolError,
                    format!("{qualified} requested interactive input (MRTR), unsupported"),
                ));
            }
            Ok(_) => {
                return Err(payload(
                    ErrorCode::ToolError,
                    format!("{qualified} returned an unsupported response type"),
                ));
            }
            Err(e) => return Err(self.map_service_error(conn, &qualified, e).await),
        };

        let content = serde_json::to_value(&result.content).unwrap_or(Value::Null);
        let text = content_text(&content);
        if result.is_error == Some(true) {
            return Err(payload(
                ErrorCode::ToolError,
                if text.is_empty() {
                    format!("{qualified} reported an error")
                } else {
                    text
                },
            ));
        }
        Ok(match result.structured_content {
            Some(v) => v,
            None => json!({ "text": text, "content": content }),
        })
    }

    async fn map_service_error(
        &self,
        conn: &Conn,
        qualified: &str,
        e: ServiceError,
    ) -> CerebroErrorPayload {
        match e {
            ServiceError::McpError(d) => {
                let code = if d.code.0 == -32602 {
                    ErrorCode::InvalidInput
                } else {
                    ErrorCode::ToolError
                };
                payload(code, format!("{qualified}: {}", d.message))
            }
            ServiceError::Timeout { .. } => {
                payload(ErrorCode::Timeout, format!("{qualified} timed out"))
            }
            other => {
                // Transporte caído: descarta la conexión para relanzarla en la siguiente.
                let mut guard = conn.live.lock().await;
                if let Some(mut live) = guard.take() {
                    let _ = live.service.close().await;
                }
                payload(
                    ErrorCode::ServerUnavailable,
                    format!("{qualified}: {other}"),
                )
            }
        }
    }

    fn not_found(&self, server: &str, tool: &str) -> CerebroErrorPayload {
        let all = self.catalog.list_tools(None).unwrap_or_default();
        let mut scored: Vec<(usize, String)> = all
            .iter()
            .filter_map(|t| {
                let same = t.server == server;
                let d = edit_distance(&t.name.to_lowercase(), &tool.to_lowercase());
                let prefix = !tool.is_empty()
                    && (t.name.to_lowercase().starts_with(&tool.to_lowercase())
                        || tool.to_lowercase().starts_with(&t.name.to_lowercase()));
                if (same && (d <= 3 || prefix)) || (!same && d <= 1) {
                    Some((if prefix { 0 } else { d }, t.qualified_name()))
                } else {
                    None
                }
            })
            .collect();
        scored.sort();
        let suggestions: Vec<String> = scored.into_iter().take(5).map(|(_, n)| n).collect();
        CerebroErrorPayload {
            code: ErrorCode::ToolNotFound,
            message: format!("tool `{server}.{tool}` not found"),
            approval_token: None,
            suggestions,
        }
    }

    // ───────────── Integración con pctx ─────────────

    /// Una config de callback por herramienta del catálogo.
    pub fn callback_configs(&self) -> Result<Vec<CallbackConfig>> {
        let mut tools = self.catalog.list_tools(None)?;
        tools.sort_by(|a, b| (&a.server, &a.name).cmp(&(&b.server, &b.name)));
        Ok(tools
            .into_iter()
            .map(|t| CallbackConfig {
                namespace: Some(t.server),
                name: t.name,
                description: Some(t.description).filter(|d| !d.is_empty()),
                input_schema: Some(t.input_schema),
                output_schema: t.output_schema,
            })
            .collect())
    }

    /// Registry nuevo **por ejecución**, con **todo** el catálogo (pctx exige que estén
    /// todos los ids del `CodeMode`).
    pub fn build_registry(
        self: &Arc<Self>,
        execution_id: &str,
        approvals: Vec<String>,
    ) -> Result<(PctxRegistry, ExecutionScope)> {
        let registry = PctxRegistry::default();
        let scope = ExecutionScope::new(execution_id);
        let ctx = CallCtx {
            execution_id: execution_id.to_string(),
            approvals,
            taint_seen: vec![],
        };
        for cfg in self.callback_configs()? {
            let id = cfg.id();
            let server = cfg.namespace.clone().unwrap_or_default();
            let tool = cfg.name.clone();
            let hub = Arc::clone(self);
            let scope = scope.clone();
            let ctx = ctx.clone();
            let cb: pctx_code_mode::registry::CallbackFn = Arc::new(move |args| {
                let hub = Arc::clone(&hub);
                let scope = scope.clone();
                let ctx = ctx.clone();
                let (server, tool) = (server.clone(), tool.clone());
                Box::pin(async move {
                    // El callback corre en un runtime efímero: el trabajo real va al principal.
                    let h = Arc::clone(&hub);
                    let jh = hub
                        .handle
                        .spawn(async move { h.call_inner(&server, &tool, args, &ctx).await });
                    match jh.await {
                        Ok((res, summary)) => {
                            scope.push(summary);
                            res.map_err(|p| p.to_callback_error())
                        }
                        Err(e) => Err(payload(
                            ErrorCode::ServerUnavailable,
                            format!("hub task failed: {e}"),
                        )
                        .to_callback_error()),
                    }
                })
            });
            registry
                .add_callback(&id, cb)
                .map_err(|e| anyhow::anyhow!("registering callback {id}: {e}"))?;
        }
        Ok((registry, scope))
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        if let Some(jh) = self.reaper.get_mut().ok().and_then(Option::take) {
            jh.abort();
        }
    }
}

async fn reaper_loop(weak: Weak<Hub>, period: Duration) {
    loop {
        tokio::time::sleep(period).await;
        let Some(hub) = weak.upgrade() else { break };
        hub.reap_idle().await;
    }
}

// ───────────────────────────── Utilidades ─────────────────────────────

fn payload(code: ErrorCode, message: String) -> CerebroErrorPayload {
    CerebroErrorPayload {
        code,
        message,
        approval_token: None,
        suggestions: vec![],
    }
}

fn value_len(v: &Value) -> u64 {
    serde_json::to_vec(v).map_or(0, |b| b.len() as u64)
}

/// JSON canónico: objetos con claves ordenadas, recursivamente.
fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let sorted: BTreeMap<&String, Value> =
                m.iter().map(|(k, v)| (k, canonical(v))).collect();
            Value::Object(sorted.into_iter().map(|(k, v)| (k.clone(), v)).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// Serializa con claves ordenadas sin depender de la feature `preserve_order`.
fn canonical_string(v: &Value) -> String {
    fn go(v: &Value, out: &mut String) {
        match v {
            Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                out.push('{');
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(k).unwrap_or_default());
                    out.push(':');
                    go(&m[*k], out);
                }
                out.push('}');
            }
            Value::Array(a) => {
                out.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    go(x, out);
                }
                out.push(']');
            }
            other => out.push_str(&serde_json::to_string(other).unwrap_or_default()),
        }
    }
    let mut s = String::new();
    go(&canonical(v), &mut s);
    s
}

fn hash_value(v: &Value) -> String {
    blake3::hash(canonical_string(v).as_bytes())
        .to_hex()
        .to_string()
}

/// Sustituye por `"***"` los valores de claves que parezcan secretos.
fn redact(v: &Value) -> Value {
    const SENSITIVE: [&str; 4] = ["token", "password", "secret", "key"];
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| {
                    let lk = k.to_lowercase();
                    if SENSITIVE.iter().any(|s| lk.contains(s)) {
                        (k.clone(), Value::String("***".into()))
                    } else {
                        (k.clone(), redact(v))
                    }
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(redact).collect()),
        other => other.clone(),
    }
}

fn content_text(content: &Value) -> String {
    content
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|i| i.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|i| i.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn tool_record(
    server: &str,
    ts_namespace: &str,
    ts_name: &str,
    t: &rmcp::model::Tool,
) -> ToolRecord {
    let input_schema = Value::Object((*t.input_schema).clone());
    let output_schema = t
        .output_schema
        .as_ref()
        .map(|s| Value::Object((**s).clone()));
    let annotations = t
        .annotations
        .as_ref()
        .and_then(|a| serde_json::to_value(a).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let description = t
        .description
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let name = t.name.to_string();

    let schema_hash = hash_value(&json!({
        "name": name,
        "description": description,
        "input_schema": input_schema,
        "output_schema": output_schema,
        "annotations": annotations,
    }));
    let raw = json!({
        "name": name, "title": t.title, "description": description,
        "inputSchema": input_schema, "outputSchema": output_schema, "annotations": annotations,
    });
    let approx_tokens = u32::try_from(value_len(&raw) / 4).unwrap_or(u32::MAX);

    ToolRecord {
        server: server.to_string(),
        ts_signature: Some(ts_signature(ts_namespace, ts_name, &input_schema)),
        name,
        ts_name: ts_name.to_string(),
        title: t.title.clone(),
        description,
        input_schema,
        output_schema,
        annotations,
        approx_tokens,
        schema_hash,
        taint_labels: vec![],
    }
}

/// Firma TS compacta de una línea: `Ns.fn({ a: string, b?: number }): Promise<any>`.
fn ts_signature(ns: &str, name: &str, input: &Value) -> String {
    let required: Vec<&str> = input
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let props = input
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| {
            let mut keys: Vec<&String> = p.keys().collect();
            keys.sort();
            keys.iter()
                .map(|k| {
                    let ty = match p[*k].get("type").and_then(Value::as_str) {
                        Some("integer" | "number") => "number",
                        Some("string") => "string",
                        Some("boolean") => "boolean",
                        Some("array") => "any[]",
                        Some("object") => "object",
                        _ => "any",
                    };
                    let opt = if required.contains(&k.as_str()) {
                        ""
                    } else {
                        "?"
                    };
                    format!("{k}{opt}: {ty}")
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    if props.is_empty() {
        format!("{ns}.{name}(): Promise<any>")
    } else {
        format!("{ns}.{name}({{ {props} }}): Promise<any>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secret_like_keys_recursively() {
        let v = json!({"user": "a", "apiKey": "x", "nested": {"Password": "p", "ok": 1}, "list": [{"token": "t"}]});
        assert_eq!(
            redact(&v),
            json!({"user": "a", "apiKey": "***", "nested": {"Password": "***", "ok": 1}, "list": [{"token": "***"}]})
        );
    }

    #[test]
    fn hash_is_independent_of_key_order() {
        assert_eq!(
            hash_value(&json!({"a": 1, "b": {"c": 2, "d": 3}})),
            hash_value(&json!({"b": {"d": 3, "c": 2}, "a": 1}))
        );
        assert_ne!(hash_value(&json!({"a": 1})), hash_value(&json!({"a": 2})));
    }

    #[test]
    fn ts_names_follow_pctx_conventions() {
        assert_eq!(Case::Pascal.sanitize("my-server"), "MyServer");
        assert_eq!(Case::Pascal.sanitize("echo"), "Echo");
        assert_eq!(Case::Camel.sanitize("create_issue"), "createIssue");
        assert_eq!(Case::Camel.sanitize("createIssue"), "createIssue");
    }

    #[test]
    fn signature_marks_optional_fields() {
        let s = ts_signature(
            "Echo",
            "echo",
            &json!({"type":"object","properties":{"text":{"type":"string"},"n":{"type":"integer"}},"required":["text"]}),
        );
        assert_eq!(s, "Echo.echo({ n?: number, text: string }): Promise<any>");
    }

    #[test]
    fn edit_distance_basic() {
        assert_eq!(edit_distance("echo", "ecoh"), 2);
        assert_eq!(edit_distance("a", "a"), 0);
    }
}
