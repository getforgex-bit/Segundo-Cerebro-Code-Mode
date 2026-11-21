//! Mocks en memoria para tests (y la feature `mock`): catálogo, búsqueda, auditoría,
//! aprobaciones y un hub falso con `Echo.echo`, `Echo.add`, `Destructive.read_item` y
//! `Destructive.delete_item`. Se sustituyen por WS2/WS3 al integrar.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use cerebro_types::*;
use pctx_code_mode::model::CallbackConfig;
use pctx_code_mode::registry::PctxRegistry;
use serde_json::{Value, json};

use crate::backend::{Backend, ExecScope, HubApi, SyncSummary};

/// Catálogo + auditoría + aprobaciones + búsqueda, todo en memoria.
#[derive(Default)]
pub struct MockStore {
    servers: Mutex<Vec<ServerRecord>>,
    tools: Mutex<Vec<ToolRecord>>,
    pub executions: Mutex<Vec<ExecutionRecord>>,
    pub tool_calls: Mutex<Vec<ToolCallRecord>>,
    /// token → (info, args_hash, status: "pending" | "approved" | "consumed")
    approvals: Mutex<Vec<(ApprovalInfo, String, &'static str)>>,
}

impl Catalog for MockStore {
    fn replace_server_tools(&self, server: &ServerRecord, tools: &[ToolRecord]) -> Result<()> {
        let mut s = self.servers.lock().unwrap();
        s.retain(|x| x.name != server.name);
        s.push(server.clone());
        let mut t = self.tools.lock().unwrap();
        t.retain(|x| x.server != server.name);
        t.extend_from_slice(tools);
        Ok(())
    }
    fn update_server_status(
        &self,
        server: &str,
        status: ServerStatus,
        err: Option<&str>,
    ) -> Result<()> {
        for s in self
            .servers
            .lock()
            .unwrap()
            .iter_mut()
            .filter(|s| s.name == server)
        {
            s.status = status;
            s.last_error = err.map(str::to_string);
        }
        Ok(())
    }
    fn get_tool(&self, server: &str, name: &str) -> Result<Option<ToolRecord>> {
        Ok(self
            .tools
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.server == server && t.name == name)
            .cloned())
    }
    fn list_tools(&self, server: Option<&str>) -> Result<Vec<ToolRecord>> {
        Ok(self
            .tools
            .lock()
            .unwrap()
            .iter()
            .filter(|t| server.is_none_or(|s| t.server == s))
            .cloned()
            .collect())
    }
    fn list_servers(&self) -> Result<Vec<ServerRecord>> {
        Ok(self.servers.lock().unwrap().clone())
    }
}

impl AuditLog for MockStore {
    fn record_tool_call(&self, rec: &ToolCallRecord) -> Result<()> {
        self.tool_calls.lock().unwrap().push(rec.clone());
        Ok(())
    }
    fn record_execution(&self, rec: &ExecutionRecord) -> Result<()> {
        self.executions.lock().unwrap().push(rec.clone());
        Ok(())
    }
}

impl ApprovalStore for MockStore {
    fn create(&self, req: &ApprovalRequest) -> Result<String> {
        let mut v = self.approvals.lock().unwrap();
        let token = format!("mock-token-{}", v.len() + 1);
        let info = ApprovalInfo {
            token: token.clone(),
            execution_id: req.execution_id.clone(),
            tool: req.tool.clone(),
            reason: req.reason.clone(),
            created_at: String::new(),
            expires_at: String::new(),
        };
        v.push((info, req.args_hash.clone(), "pending"));
        Ok(token)
    }
    fn approve(&self, token: &str) -> Result<bool> {
        let mut v = self.approvals.lock().unwrap();
        Ok(v.iter_mut()
            .find(|(i, _, st)| i.token == token && *st == "pending")
            .map(|e| e.2 = "approved")
            .is_some())
    }
    fn consume(
        &self,
        token: &str,
        tool: &str,
        args_hash: &str,
        mode: ApprovalMode,
    ) -> Result<bool> {
        let mut v = self.approvals.lock().unwrap();
        let Some(e) = v
            .iter_mut()
            .find(|(i, h, _)| i.token == token && i.tool == tool && h == args_hash)
        else {
            return Ok(false);
        };
        let ok = e.2 == "approved" || (e.2 == "pending" && mode == ApprovalMode::ClientConfirm);
        if ok {
            e.2 = "consumed";
        }
        Ok(ok)
    }
    fn list_pending(&self) -> Result<Vec<ApprovalInfo>> {
        Ok(self
            .approvals
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.2 == "pending")
            .map(|e| e.0.clone())
            .collect())
    }
}

impl ToolSearch for MockStore {
    /// Coincidencia por términos en `server`, `name` y `description` (solo para tests).
    fn search(&self, query: &str, opts: &SearchOpts) -> Result<Vec<ToolHit>> {
        let terms: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();
        let mut hits: Vec<ToolHit> = self
            .tools
            .lock()
            .unwrap()
            .iter()
            .filter(|t| opts.server.as_deref().is_none_or(|s| t.server == s))
            .filter_map(|t| {
                let hay = format!("{} {} {}", t.server, t.name, t.description).to_lowercase();
                let score = terms.iter().filter(|w| hay.contains(w.as_str())).count();
                (score > 0).then(|| ToolHit {
                    ts_namespace: pascal(&t.server),
                    tool: t.clone(),
                    score: score as f64,
                })
            })
            .collect();
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(opts.limit);
        Ok(hits)
    }
}

fn pascal(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Formato de CONTRACTS §4 (mock): `Ns.fn — desc [marca]`.
pub fn format_hits(hits: &[ToolHit], detail: Detail) -> String {
    hits.iter()
        .map(|h| {
            let t = &h.tool;
            let id = format!("{}.{}", h.ts_namespace, t.ts_name);
            let mark = if t.read_only_hint() == Some(true) {
                "[read]"
            } else if t.destructive_hint() == Some(true) {
                "[destructive]"
            } else {
                "[write]"
            };
            match detail {
                Detail::Names => id,
                Detail::Summary => format!("{id} — {} {mark}", t.description),
                Detail::Full => format!(
                    "{id} — {} {mark}\n  {}",
                    t.description,
                    t.ts_signature.clone().unwrap_or_default()
                ),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool(server: &str, name: &str, ts: &str, desc: &str, schema: Value, hints: Value) -> ToolRecord {
    ToolRecord {
        server: server.into(),
        name: name.into(),
        ts_name: ts.into(),
        title: None,
        description: desc.into(),
        input_schema: schema,
        output_schema: None,
        annotations: hints,
        ts_signature: None,
        approx_tokens: 50,
        schema_hash: blake3::hash(format!("{server}.{name}").as_bytes())
            .to_hex()
            .to_string(),
        taint_labels: vec![],
    }
}

fn mock_catalog() -> Vec<(ServerRecord, Vec<ToolRecord>)> {
    let srv = |name: &str| ServerRecord {
        name: name.into(),
        ts_namespace: pascal(name),
        status: ServerStatus::Ok,
        last_error: None,
        protocol_version: None,
        instructions: None,
        catalog_hash: None,
    };
    let text = json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"]});
    let nums = json!({"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}},"required":["a","b"]});
    let id = json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]});
    let ro = json!({"readOnlyHint": true});
    vec![
        (
            srv("echo"),
            vec![
                tool(
                    "echo",
                    "echo",
                    "echo",
                    "Echo the given text back",
                    text,
                    ro.clone(),
                ),
                tool("echo", "add", "add", "Add two numbers", nums, ro.clone()),
            ],
        ),
        (
            srv("destructive"),
            vec![
                tool(
                    "destructive",
                    "read_item",
                    "readItem",
                    "Read an item by id",
                    id.clone(),
                    ro,
                ),
                tool(
                    "destructive",
                    "delete_item",
                    "deleteItem",
                    "Delete an item by id",
                    id,
                    json!({"destructiveHint": true}),
                ),
            ],
        ),
    ]
}

#[derive(Default)]
struct Scope {
    calls: Mutex<Vec<CallSummary>>,
}
impl ExecScope for Scope {
    fn calls(&self) -> Vec<CallSummary> {
        self.calls.lock().unwrap().clone()
    }
    fn raw_bytes(&self) -> u64 {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.result_bytes)
            .sum()
    }
}
struct ScopeHandle(Arc<Scope>);
impl ExecScope for ScopeHandle {
    fn calls(&self) -> Vec<CallSummary> {
        self.0.calls()
    }
    fn raw_bytes(&self) -> u64 {
        self.0.raw_bytes()
    }
}

/// Hub falso: las herramientas responden en proceso, sin servidores downstream.
pub struct MockHub {
    store: Arc<dyn Catalog>,
    approvals: Arc<dyn ApprovalStore>,
    mode: ApprovalMode,
}

impl MockHub {
    pub fn new(
        store: Arc<dyn Catalog>,
        approvals: Arc<dyn ApprovalStore>,
        mode: ApprovalMode,
    ) -> Self {
        Self {
            store,
            approvals,
            mode,
        }
    }
}

fn call_tool(
    server: &str,
    name: &str,
    args: &Value,
    approvals: &[String],
    store: &dyn ApprovalStore,
    mode: ApprovalMode,
    execution_id: &str,
) -> std::result::Result<Value, CerebroErrorPayload> {
    match (server, name) {
        ("echo", "echo") => Ok(json!({"text": args["text"]})),
        ("echo", "add") => Ok(
            json!({"sum": args["a"].as_f64().unwrap_or(0.0) + args["b"].as_f64().unwrap_or(0.0)}),
        ),
        ("destructive", "read_item") => Ok(json!({"id": args["id"], "value": "stored"})),
        ("destructive", "delete_item") => {
            let tool = "destructive.delete_item";
            let args_hash = blake3::hash(args.to_string().as_bytes())
                .to_hex()
                .to_string();
            for t in approvals {
                if store.consume(t, tool, &args_hash, mode).unwrap_or(false) {
                    return Ok(json!({"deleted": args["id"]}));
                }
            }
            let token = store
                .create(&ApprovalRequest {
                    execution_id: execution_id.into(),
                    tool: tool.into(),
                    args_hash,
                    reason: "destructive".into(),
                    ttl_seconds: 600,
                })
                .map_err(|e| CerebroErrorPayload {
                    code: ErrorCode::ToolError,
                    message: e.to_string(),
                    approval_token: None,
                    suggestions: vec![],
                })?;
            Err(CerebroErrorPayload {
                code: ErrorCode::NeedsApproval,
                message: format!(
                    "Approval required: ask the user to run `cerebro approve {token}` in an interactive terminal, then retry with approvals: [\"{token}\"]"
                ),
                approval_token: Some(token),
                suggestions: vec![],
            })
        }
        _ => Err(CerebroErrorPayload {
            code: ErrorCode::ToolNotFound,
            message: format!("{server}.{name}"),
            approval_token: None,
            suggestions: vec![],
        }),
    }
}

impl HubApi for MockHub {
    async fn sync_catalog(&self) -> anyhow::Result<SyncSummary> {
        let mut sum = SyncSummary::default();
        for (s, t) in mock_catalog() {
            self.store.replace_server_tools(&s, &t)?;
            sum.servers_ok.push(s.name.clone());
            sum.tools += t.len();
        }
        Ok(sum)
    }

    fn callback_configs(&self) -> anyhow::Result<Vec<CallbackConfig>> {
        Ok(self
            .store
            .list_tools(None)?
            .into_iter()
            .map(|t| CallbackConfig {
                name: t.name,
                namespace: Some(t.server),
                description: Some(t.description),
                input_schema: Some(t.input_schema),
                output_schema: t.output_schema,
            })
            .collect())
    }

    fn build_registry(
        &self,
        execution_id: &str,
        approvals: Vec<String>,
    ) -> anyhow::Result<(PctxRegistry, Box<dyn ExecScope>)> {
        let registry = PctxRegistry::default();
        let scope = Arc::new(Scope::default());
        for t in self.store.list_tools(None)? {
            let id = format!("{}__{}", t.server, t.name);
            let (server, name, scope, approvals) = (
                t.server.clone(),
                t.name.clone(),
                scope.clone(),
                approvals.clone(),
            );
            let (store, mode, exec_id) =
                (self.approvals.clone(), self.mode, execution_id.to_string());
            registry.add_callback(
                &id,
                Arc::new(move |args: Option<Value>| {
                    let (server, name, scope, approvals) = (
                        server.clone(),
                        name.clone(),
                        scope.clone(),
                        approvals.clone(),
                    );
                    let (store, exec_id) = (store.clone(), exec_id.clone());
                    Box::pin(async move {
                        let started = Instant::now();
                        let r = call_tool(
                            &server,
                            &name,
                            &args.unwrap_or(Value::Null),
                            &approvals,
                            store.as_ref(),
                            mode,
                            &exec_id,
                        );
                        scope.calls.lock().unwrap().push(CallSummary {
                            tool: format!("{server}.{name}"),
                            ms: started.elapsed().as_millis() as u64,
                            ok: r.is_ok(),
                            decision: CallDecision::Allowed,
                            result_bytes: r
                                .as_ref()
                                .map(|v| v.to_string().len() as u64)
                                .unwrap_or(0),
                        });
                        r.map_err(|p| p.to_callback_error())
                    })
                }),
            )?;
        }
        Ok((registry, Box::new(ScopeHandle(scope))))
    }

    async fn shutdown(&self) {}
}

/// Backend completo con mocks. Devuelve también el store para inspeccionar la auditoría.
pub fn mock_backend(mode: ApprovalMode) -> (Backend<MockHub>, Arc<MockStore>) {
    let store = Arc::new(MockStore::default());
    let backend = Backend {
        catalog: store.clone(),
        search: store.clone(),
        audit: store.clone(),
        format_hits,
        hub: Arc::new(MockHub::new(store.clone(), store.clone(), mode)),
    };
    (backend, store)
}

/// Backend con el `Store` y la búsqueda REALES (WS2) y el hub falso: para probar el
/// cableado de persistencia sin el hub de WS3.
pub fn store_backend(store: cerebro_store::Store, mode: ApprovalMode) -> Backend<MockHub> {
    let store = Arc::new(store);
    Backend {
        catalog: store.clone(),
        search: Arc::new(cerebro_search::SqliteToolSearch::new((*store).clone())),
        audit: store.clone(),
        format_hits: cerebro_search::format_hits,
        hub: Arc::new(MockHub::new(store.clone(), store, mode)),
    }
}
