//! Mocks de `Catalog`, `AuditLog` y `ApprovalStore` (hasta integrar `cerebro_store`).
#![allow(dead_code, unreachable_pub, clippy::pedantic)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cerebro_hub::{DownstreamServer, Hub, HubConfig};
use cerebro_policy::{Policy, PolicyConfig};
use cerebro_types::*;

#[derive(Default)]
pub struct MockCatalog {
    servers: Mutex<HashMap<String, ServerRecord>>,
    tools: Mutex<HashMap<String, Vec<ToolRecord>>>,
}

impl Catalog for MockCatalog {
    fn replace_server_tools(&self, server: &ServerRecord, tools: &[ToolRecord]) -> Result<()> {
        self.servers
            .lock()
            .unwrap()
            .insert(server.name.clone(), server.clone());
        self.tools
            .lock()
            .unwrap()
            .insert(server.name.clone(), tools.to_vec());
        Ok(())
    }
    fn update_server_status(
        &self,
        server: &str,
        status: ServerStatus,
        err: Option<&str>,
    ) -> Result<()> {
        let mut s = self.servers.lock().unwrap();
        let r = s
            .get_mut(server)
            .ok_or_else(|| anyhow::anyhow!("no such server"))?;
        r.status = status;
        r.last_error = err.map(str::to_string);
        Ok(())
    }
    fn get_tool(&self, server: &str, name: &str) -> Result<Option<ToolRecord>> {
        Ok(self
            .tools
            .lock()
            .unwrap()
            .get(server)
            .and_then(|t| t.iter().find(|t| t.name == name).cloned()))
    }
    fn list_tools(&self, server: Option<&str>) -> Result<Vec<ToolRecord>> {
        let t = self.tools.lock().unwrap();
        Ok(t.iter()
            .filter(|(k, _)| server.is_none_or(|s| s == k.as_str()))
            .flat_map(|(_, v)| v.clone())
            .collect())
    }
    fn list_servers(&self) -> Result<Vec<ServerRecord>> {
        Ok(self.servers.lock().unwrap().values().cloned().collect())
    }
}

#[derive(Default)]
pub struct MockAudit {
    pub calls: Mutex<Vec<ToolCallRecord>>,
}
impl AuditLog for MockAudit {
    fn record_tool_call(&self, rec: &ToolCallRecord) -> Result<()> {
        self.calls.lock().unwrap().push(rec.clone());
        Ok(())
    }
    fn record_execution(&self, _rec: &ExecutionRecord) -> Result<()> {
        Ok(())
    }
}

/// Un solo uso: pendiente o aprobado → consume → gastado.
#[derive(Default)]
pub struct MockApprovals {
    items: Mutex<HashMap<String, (ApprovalRequest, bool)>>, // (req, usado)
    n: Mutex<u32>,
}
impl ApprovalStore for MockApprovals {
    fn create(&self, req: &ApprovalRequest) -> Result<String> {
        let mut n = self.n.lock().unwrap();
        *n += 1;
        let tok = format!("tok_{n}");
        self.items
            .lock()
            .unwrap()
            .insert(tok.clone(), (req.clone(), false));
        Ok(tok)
    }
    fn approve(&self, token: &str) -> Result<bool> {
        Ok(self.items.lock().unwrap().contains_key(token))
    }
    fn consume(&self, token: &str, tool: &str, args_hash: &str) -> Result<bool> {
        let mut items = self.items.lock().unwrap();
        match items.get_mut(token) {
            Some((req, used)) if !*used && req.tool == tool && req.args_hash == args_hash => {
                *used = true;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

pub struct Env {
    pub hub: Arc<Hub>,
    pub catalog: Arc<MockCatalog>,
    pub audit: Arc<MockAudit>,
    pub approvals: Arc<MockApprovals>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn fixture(name: &str) -> DownstreamServer {
    let js = repo_root().join(format!("packages/fixtures/dist/{name}.js"));
    assert!(
        js.exists(),
        "falta {js:?}: ejecuta `pnpm --dir packages install && pnpm --dir packages --filter @cerebro/fixtures build`"
    );
    DownstreamServer {
        name: name.into(),
        command: "node".into(),
        args: vec![js.to_string_lossy().into()],
        env: Default::default(),
        cwd: None,
    }
}

pub fn script(name: &str, script: &str) -> DownstreamServer {
    let js = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/support")
        .join(script);
    DownstreamServer {
        name: name.into(),
        command: "node".into(),
        args: vec![js.to_string_lossy().into()],
        env: Default::default(),
        cwd: None,
    }
}

pub fn env_with(
    servers: Vec<DownstreamServer>,
    policy: serde_json::Value,
    cfg: impl FnOnce(&mut HubConfig),
) -> Env {
    let catalog = Arc::new(MockCatalog::default());
    let audit = Arc::new(MockAudit::default());
    let approvals = Arc::new(MockApprovals::default());
    let mut hc = HubConfig {
        servers,
        call_timeout: Duration::from_secs(10),
        idle_shutdown: Duration::ZERO,
    };
    cfg(&mut hc);
    let policy = Policy::new(serde_json::from_value::<PolicyConfig>(policy).unwrap());
    let hub = Hub::new(
        hc,
        catalog.clone(),
        audit.clone(),
        approvals.clone(),
        policy,
    );
    Env {
        hub,
        catalog,
        audit,
        approvals,
    }
}

pub fn ctx(id: &str, approvals: &[&str]) -> CallCtx {
    CallCtx {
        execution_id: id.into(),
        approvals: approvals.iter().map(|s| (*s).to_string()).collect(),
        taint_seen: vec![],
    }
}
