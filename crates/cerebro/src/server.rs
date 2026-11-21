//! Servidor MCP `cerebro`: implementa `rmcp::ServerHandler` a mano (6 herramientas fijas).

use std::sync::Arc;
use std::time::{Duration, Instant};

use cerebro_types::{CerebroErrorPayload, ErrorCode, ExecutionRecord, SearchOpts, ServerStatus};
use pctx_code_mode::config::ToolDisclosure;
use pctx_code_mode::model::{FunctionId, GetFunctionDetailsInput};
use pctx_code_mode::{CodeMode, ExecuteTypescriptOptions};
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use serde::Serialize;
use serde_json::Value;

use crate::backend::{Backend, HubApi};
use crate::config::Limits;
use crate::tools::{self, BRAIN_STUB};

/// Recorta `s` a `max` bytes sin partir un carácter. Devuelve si hubo recorte.
pub fn truncate_utf8(s: &str, max: usize) -> (String, bool) {
    if s.len() <= max {
        return (s.to_string(), false);
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    (s[..end].to_string(), true)
}

#[derive(Serialize)]
struct ExecuteResponse {
    success: bool,
    output: Value,
    stdout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
    calls: Vec<cerebro_types::CallSummary>,
    truncated: bool,
}

pub struct CerebroServer<H: HubApi> {
    backend: Arc<Backend<H>>,
    code_mode: Arc<CodeMode>,
    limits: Limits,
}

impl<H: HubApi> Clone for CerebroServer<H> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            code_mode: self.code_mode.clone(),
            limits: self.limits,
        }
    }
}

impl<H: HubApi> CerebroServer<H> {
    /// Arranque: `sync_catalog` → `CodeMode::add_callback` por cada config.
    pub async fn start(backend: Backend<H>, limits: Limits) -> anyhow::Result<Self> {
        let report = backend.hub.sync_catalog().await?;
        tracing::info!(
            ok = report.servers_ok.len(),
            failed = report.servers_failed.len(),
            tools = report.tools,
            "catálogo sincronizado"
        );
        for (s, e) in &report.servers_failed {
            tracing::warn!(server = %s, error = %e, "servidor downstream no disponible");
        }
        let mut cm = CodeMode::default();
        for cfg in backend.hub.callback_configs()? {
            match cm.add_callback(&cfg) {
                Ok(warnings) => {
                    for w in warnings {
                        tracing::warn!(tool = %cfg.id(), "{w}");
                    }
                }
                Err(e) => tracing::warn!(tool = %cfg.id(), error = %e, "callback no registrado"),
            }
        }
        Ok(Self {
            backend: Arc::new(backend),
            code_mode: Arc::new(cm),
            limits,
        })
    }

    pub async fn shutdown(&self) {
        self.backend.hub.shutdown().await;
    }

    fn text_ok(text: impl Into<String>) -> CallToolResponse {
        CallToolResult::success(vec![ContentBlock::text(text)]).into()
    }

    fn text_err(text: impl Into<String>) -> CallToolResponse {
        CallToolResult::error(vec![ContentBlock::text(text)]).into()
    }

    fn search(&self, args: tools::SearchArgs) -> CallToolResponse {
        let defaults = SearchOpts::default();
        let detail = args.detail.unwrap_or(defaults.detail);
        let opts = SearchOpts {
            detail,
            server: args.server,
            limit: args.limit.unwrap_or(defaults.limit).clamp(1, 25),
        };
        match self.backend.search.search(&args.query, &opts) {
            Ok(hits) if hits.is_empty() => Self::text_ok("No matching functions."),
            Ok(hits) => Self::text_ok((self.backend.format_hits)(&hits, detail)),
            Err(e) => Self::text_err(format!("search failed: {e}")),
        }
    }

    /// Acepta `Namespace.fn` y `server.tool` (traducido vía catálogo).
    fn translate_function(&self, raw: &str) -> Option<FunctionId> {
        let (m, f) = raw.split_once('.')?;
        if let Ok(Some(t)) = self.backend.catalog.get_tool(m, f)
            && let Ok(servers) = self.backend.catalog.list_servers()
            && let Some(s) = servers.iter().find(|s| s.name == t.server)
        {
            return Some(FunctionId {
                mod_name: s.ts_namespace.clone(),
                fn_name: t.ts_name,
            });
        }
        Some(FunctionId {
            mod_name: m.to_string(),
            fn_name: f.to_string(),
        })
    }

    fn details(&self, args: tools::DetailsArgs) -> CallToolResponse {
        let mut ids = Vec::new();
        for f in &args.functions {
            match self.translate_function(f) {
                Some(id) => ids.push(id),
                None => {
                    return Self::text_err(format!(
                        "invalid function id {f:?}: expected 'Namespace.fn' or 'server.tool'"
                    ));
                }
            }
        }
        let out = self
            .code_mode
            .get_function_details(GetFunctionDetailsInput { functions: ids });
        Self::text_ok(out.code)
    }

    async fn execute(&self, args: tools::ExecuteArgs) -> CallToolResponse {
        let started = Instant::now();
        let execution_id = ulid::Ulid::generate().to_string().to_lowercase();
        let code_hash = blake3::hash(args.code.as_bytes()).to_hex().to_string();
        let max = self.limits.max_result_bytes;

        let built = self
            .backend
            .hub
            .build_registry(&execution_id, args.approvals);
        let (registry, scope) = match built {
            Ok(x) => x,
            Err(e) => return Self::text_err(format!("cannot start execution: {e}")),
        };

        let cm = self.code_mode.clone();
        let opts = ExecuteTypescriptOptions {
            timeout: Some(Duration::from_millis(self.limits.timeout_ms)),
            heap_limit_bytes: usize::try_from(self.limits.heap_limit_mb)
                .ok()
                .and_then(|mb| mb.checked_mul(1024 * 1024)),
        };
        let code = args.code;
        let span = tracing::Span::current();
        let handle = tokio::task::spawn_blocking(move || {
            let _g = span.enter();
            // Deno no es `Send`: runtime current_thread propio (como pctx_mcp_server).
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| anyhow::anyhow!("no se pudo crear el runtime: {e}"))?;
            rt.block_on(async {
                cm.execute_typescript_with_options(
                    &code,
                    ToolDisclosure::Catalog,
                    Some(registry),
                    opts,
                )
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))
            })
        });
        // Timeout y heap los aplica el propio executor (parche CEREBRO-PATCH de WS1): el hilo
        // de Deno se detiene de verdad al vencer el plazo.
        let join_res = handle.await;
        let (success, output, stdout, stderr, status) = match join_res {
            Err(join) => (
                false,
                Value::Null,
                String::new(),
                format!("execution task failed: {join}"),
                "error",
            ),
            Ok(Err(e)) => (false, Value::Null, String::new(), e.to_string(), "error"),
            Ok(Ok(o)) => {
                let timed_out = !o.success && o.stderr.contains("Execution exceeded");
                let stderr = if timed_out {
                    CerebroErrorPayload {
                        code: ErrorCode::Timeout,
                        message: format!(
                            "execution exceeded {} ms and was stopped",
                            self.limits.timeout_ms
                        ),
                        approval_token: None,
                        suggestions: vec![],
                    }
                    .to_callback_error()
                } else {
                    o.stderr.clone()
                };
                let status = if timed_out {
                    "timeout"
                } else if o.success {
                    "ok"
                } else {
                    "error"
                };
                (
                    o.success,
                    o.output.clone().unwrap_or(Value::Null),
                    o.stdout.clone(),
                    stderr,
                    status,
                )
            }
        };

        let mut truncated = false;
        let output = {
            let s = serde_json::to_string(&output).unwrap_or_default();
            if s.len() > max {
                truncated = true;
                Value::String(truncate_utf8(&s, max).0)
            } else {
                output
            }
        };
        let (stdout, t) = truncate_utf8(&stdout, max);
        truncated |= t;
        let error = if success || stderr.trim().is_empty() {
            None
        } else {
            Some(match CerebroErrorPayload::parse_from(&stderr) {
                Some(p) => serde_json::to_value(p).unwrap_or(Value::Null),
                None => Value::String(truncate_utf8(&stderr, max).0),
            })
        };
        let calls = scope.calls();
        let resp = ExecuteResponse {
            success,
            output,
            stdout,
            error,
            calls: calls.clone(),
            truncated,
        };
        let body = serde_json::to_string(&resp)
            .unwrap_or_else(|e| format!("{{\"success\":false,\"error\":\"{e}\"}}"));

        let rec = ExecutionRecord {
            execution_id,
            code_hash,
            status: status.into(),
            error: resp.error.as_ref().map(Value::to_string),
            tool_calls: calls.len() as u32,
            raw_bytes_processed: scope.raw_bytes(),
            result_bytes: body.len() as u64,
            duration_ms: started.elapsed().as_millis() as u64,
        };
        if let Err(e) = self.backend.audit.record_execution(&rec) {
            tracing::warn!(error = %e, "no se pudo auditar la ejecución");
        }
        Self::text_ok(body)
    }

    fn primer(&self) -> CallToolResponse {
        let servers = match self.backend.catalog.list_servers() {
            Ok(s) => s,
            Err(e) => return Self::text_err(format!("catalog error: {e}")),
        };
        let mut out = String::from("Tool servers:\n");
        if servers.is_empty() {
            out.push_str("(none configured)\n");
        }
        for s in &servers {
            let n = self
                .backend
                .catalog
                .list_tools(Some(&s.name))
                .map(|t| t.len())
                .unwrap_or(0);
            let st = match s.status {
                ServerStatus::Ok => "ok",
                ServerStatus::Error => "error",
                ServerStatus::Disabled => "disabled",
                ServerStatus::Unknown => "unknown",
            };
            out.push_str(&format!(
                "- {} (namespace {}): {st}, {n} tools",
                s.name, s.ts_namespace
            ));
            if let Some(e) = &s.last_error {
                out.push_str(&format!(" — {}", truncate_utf8(e, 200).0));
            }
            out.push('\n');
        }
        out.push_str("Memory: not implemented yet (phase 2).");
        Self::text_ok(out)
    }
}

fn parse<T: serde::de::DeserializeOwned>(
    args: Option<rmcp::model::JsonObject>,
) -> Result<T, McpError> {
    serde_json::from_value(Value::Object(args.unwrap_or_default()))
        .map_err(|e| McpError::invalid_params(format!("invalid arguments: {e}"), None))
}

impl<H: HubApi> ServerHandler for CerebroServer<H> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(tools::INSTRUCTIONS)
            .with_server_info(Implementation::new("cerebro", env!("CARGO_PKG_VERSION")))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(tools::all()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let args = request.arguments;
        Ok(match request.name.as_ref() {
            tools::SEARCH_FUNCTIONS => self.search(parse(args)?),
            tools::GET_FUNCTION_DETAILS => self.details(parse(args)?),
            tools::EXECUTE_TYPESCRIPT => self.execute(parse(args)?).await,
            tools::CEREBRO_RECALL => {
                let _: tools::RecallArgs = parse(args)?;
                Self::text_ok(BRAIN_STUB)
            }
            tools::CEREBRO_REMEMBER => {
                let _: tools::RememberArgs = parse(args)?;
                Self::text_ok(BRAIN_STUB)
            }
            tools::CEREBRO_PRIMER => {
                let _: tools::PrimerArgs = parse(args)?;
                self.primer()
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown tool: {other}"),
                    None,
                ));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::mock_backend;
    use cerebro_types::ApprovalStore;
    use rmcp::ServiceExt;
    use rmcp::model::CallToolRequestParams;
    use serde_json::json;

    async fn connect() -> (
        rmcp::service::RunningService<rmcp::RoleClient, ()>,
        Arc<crate::mock::MockStore>,
    ) {
        connect_mode(cerebro_types::ApprovalMode::ClientConfirm).await
    }

    async fn connect_mode(
        mode: cerebro_types::ApprovalMode,
    ) -> (
        rmcp::service::RunningService<rmcp::RoleClient, ()>,
        Arc<crate::mock::MockStore>,
    ) {
        let (backend, store) = mock_backend(mode);
        let server = CerebroServer::start(backend, Limits::default())
            .await
            .unwrap();
        let (s_io, c_io) = tokio::io::duplex(1 << 20);
        tokio::spawn(async move {
            let running = server.serve(s_io).await.unwrap();
            let _ = running.waiting().await;
        });
        (().serve(c_io).await.unwrap(), store)
    }

    fn args(v: Value) -> rmcp::model::JsonObject {
        v.as_object().unwrap().clone()
    }

    async fn call(
        c: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
        name: &'static str,
        a: Value,
    ) -> String {
        let r = c
            .call_tool(CallToolRequestParams::new(name).with_arguments(args(a)))
            .await
            .unwrap();
        serde_json::to_string(&r).unwrap()
    }

    #[tokio::test]
    async fn lists_six_tools_and_measures_size() {
        let (c, _) = connect().await;
        let tools = c.list_all_tools().await.unwrap();
        assert_eq!(tools.len(), 6);
        let bytes = serde_json::to_vec(&tools).unwrap().len();
        eprintln!("TOOLS_LIST_BYTES={bytes} approx_tokens={}", bytes / 4);
        assert!(bytes <= 8000, "tools/list pesa {bytes} bytes");
    }

    #[tokio::test]
    async fn search_and_details() {
        let (c, _) = connect().await;
        let s = call(&c, "search_functions", json!({"query": "echo text"})).await;
        assert!(s.contains("Echo.echo"), "{s}");
        let d = call(
            &c,
            "get_function_details",
            json!({"functions": ["echo.echo", "Echo.add"]}),
        )
        .await;
        assert!(d.contains("echo") && d.contains("add"), "{d}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn execute_two_calls_and_approval_flow() {
        let (c, store) = connect().await;
        let r = call(&c, "execute_typescript", json!({"code":
            "async function run(){ const a = await Echo.echo({text:'a'}); const b = await Echo.add({a:1,b:2}); return {a, b}; }"})).await;
        assert!(r.contains("\\\"success\\\":true"), "{r}");
        assert_eq!(store.executions.lock().unwrap().len(), 1);
        assert_eq!(store.executions.lock().unwrap()[0].tool_calls, 2);

        let r = call(
            &c,
            "execute_typescript",
            json!({"code":
            "async function run(){ return await Destructive.deleteItem({id:'1'}); }"}),
        )
        .await;
        assert!(
            r.contains("NEEDS_APPROVAL") && r.contains("mock-token-1"),
            "{r}"
        );

        let r = call(
            &c,
            "execute_typescript",
            json!({"approvals":["mock-token-1"], "code":
            "async function run(){ return await Destructive.deleteItem({id:'1'}); }"}),
        )
        .await;
        assert!(
            r.contains("\\\"success\\\":true") && r.contains("deleted"),
            "{r}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn out_of_band_requires_human_approval() {
        let (c, store) = connect_mode(cerebro_types::ApprovalMode::OutOfBand).await;
        let code = "async function run(){ return await Destructive.deleteItem({id:'1'}); }";
        let r = call(&c, "execute_typescript", json!({"code": code})).await;
        assert!(
            r.contains("NEEDS_APPROVAL") && r.contains("cerebro approve mock-token-1"),
            "{r}"
        );
        // La IA reenvía el token sin aprobación humana: sigue denegado.
        let r = call(
            &c,
            "execute_typescript",
            json!({"approvals":["mock-token-1"], "code": code}),
        )
        .await;
        assert!(r.contains("NEEDS_APPROVAL"), "{r}");
        assert_eq!(store.list_pending().unwrap().len(), 2);
        // El humano aprueba fuera de banda.
        assert!(store.approve("mock-token-1").unwrap());
        let r = call(
            &c,
            "execute_typescript",
            json!({"approvals":["mock-token-1"], "code": code}),
        )
        .await;
        assert!(r.contains("deleted"), "{r}");
        // Un solo uso.
        let r = call(
            &c,
            "execute_typescript",
            json!({"approvals":["mock-token-1"], "code": code}),
        )
        .await;
        assert!(r.contains("NEEDS_APPROVAL"), "{r}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn type_error_does_not_hang() {
        let (c, _) = connect().await;
        let r = call(
            &c,
            "execute_typescript",
            json!({"code":
            "async function run(){ const x: number = 'no'; return x; }"}),
        )
        .await;
        assert!(r.contains("\\\"success\\\":false"), "{r}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn real_store_and_search() {
        let store = cerebro_store::Store::open_in_memory().unwrap();
        let server = CerebroServer::start(
            crate::mock::store_backend(store, cerebro_types::ApprovalMode::OutOfBand),
            Limits::default(),
        )
        .await
        .unwrap();
        let (s_io, c_io) = tokio::io::duplex(1 << 20);
        tokio::spawn(async move {
            let _ = server.serve(s_io).await.unwrap().waiting().await;
        });
        let c = ().serve(c_io).await.unwrap();
        let s = call(&c, "search_functions", json!({"query": "echo text"})).await;
        let first = s.find("Echo.echo").expect(&s);
        assert!(s.find("Echo.add").is_none_or(|i| i > first), "{s}");
        let r = call(
            &c,
            "execute_typescript",
            json!({"code":
            "async function run(){ return await Echo.add({a:2,b:3}); }"}),
        )
        .await;
        assert!(r.contains("\\\"success\\\":true"), "{r}");
    }

    #[tokio::test]
    async fn stubs_and_primer() {
        let (c, _) = connect().await;
        assert!(
            call(&c, "cerebro_recall", json!({"query":"x"}))
                .await
                .contains("phase 2")
        );
        let p = call(&c, "cerebro_primer", json!({})).await;
        assert!(p.contains("echo") && p.contains("2 tools"), "{p}");
    }

    #[test]
    fn truncates_on_char_boundary() {
        let (s, t) = truncate_utf8("ñññ", 3);
        assert!(t && s == "ñ");
    }
}
