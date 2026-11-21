//! Mocks de `Catalog`, `AuditLog` y `ApprovalStore` (hasta integrar `cerebro_store`).
#![allow(dead_code, unreachable_pub, clippy::pedantic)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cerebro_hub::{DownstreamServer, Hub, HubConfig};
use cerebro_policy::{Policy, PolicyConfig};
use cerebro_store::Store;
use cerebro_types::*;

/// Escribe en el `Store` real y guarda copia en memoria para inspeccionar en los tests.
pub struct TeeAudit {
    store: Store,
    pub calls: Mutex<Vec<ToolCallRecord>>,
}
impl AuditLog for TeeAudit {
    fn record_tool_call(&self, rec: &ToolCallRecord) -> Result<()> {
        self.store.record_tool_call(rec)?;
        self.calls.lock().unwrap().push(rec.clone());
        Ok(())
    }
    fn record_execution(&self, rec: &ExecutionRecord) -> Result<()> {
        self.store.record_execution(rec)
    }
}

pub struct Env {
    pub hub: Arc<Hub>,
    pub catalog: Store,
    pub audit: Arc<TeeAudit>,
    pub approvals: Store,
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
    let store = Store::open_in_memory().unwrap();
    let audit = Arc::new(TeeAudit {
        store: store.clone(),
        calls: Mutex::default(),
    });
    let mut hc = HubConfig {
        servers,
        call_timeout: Duration::from_secs(10),
        idle_shutdown: Duration::ZERO,
        approval_mode: Default::default(),
    };
    cfg(&mut hc);
    let policy = Policy::new(serde_json::from_value::<PolicyConfig>(policy).unwrap());
    let hub = Hub::new(
        hc,
        Arc::new(store.clone()),
        audit.clone(),
        Arc::new(store.clone()),
        policy,
    );
    Env {
        hub,
        catalog: store.clone(),
        audit,
        approvals: store,
    }
}

pub fn ctx(id: &str, approvals: &[&str]) -> CallCtx {
    CallCtx {
        execution_id: id.into(),
        approvals: approvals.iter().map(|s| (*s).to_string()).collect(),
        taint_seen: vec![],
    }
}
