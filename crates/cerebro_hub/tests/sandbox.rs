#![allow(clippy::pedantic)]
//! Test clave de la arquitectura: TS en el sandbox de pctx llama a `Echo.echo()` a través del hub.
mod common;

use std::sync::Arc;

use cerebro_types::*;
use common::*;
use pctx_code_mode::{CodeMode, config::ToolDisclosure};
use serde_json::json;

/// Igual que pctx_mcp_server: Deno no es `Send`, así que corre en `spawn_blocking` con su
/// propio runtime `current_thread` (los callbacks viven en él; el hub salta al principal).
async fn run_ts(
    cm: &CodeMode,
    code: &str,
    registry: pctx_code_mode::registry::PctxRegistry,
) -> pctx_code_mode::model::ExecuteTypescriptOutput {
    let cm = cm.clone();
    let code = code.to_string();
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(cm.execute_typescript(&code, ToolDisclosure::default(), Some(registry)))
    })
    .await
    .unwrap()
    .unwrap()
}

async fn code_mode(e: &Env) -> CodeMode {
    let mut cm = CodeMode::default();
    for cfg in e.hub.callback_configs().unwrap() {
        cm.add_callback(&cfg).unwrap();
    }
    cm
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ts_calls_echo_through_the_hub() {
    let e = env_with(
        vec![fixture("echo"), fixture("destructive")],
        json!({}),
        |_| {},
    );
    e.hub.sync_catalog().await.unwrap();
    let cm = code_mode(&e).await;
    let (registry, scope) = e.hub.build_registry("exec1", vec![]).unwrap();
    let code = r#"async function run() { const r = await Echo.echo({text:"hola"}); return r; }"#;
    let out = run_ts(&cm, code, registry).await;
    assert!(out.success, "stderr: {}", out.stderr);
    assert_eq!(out.output, Some(json!({"text": "hola"})));
    let calls = scope.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool, "echo.echo");
    assert!(calls[0].ok);
    assert!(scope.raw_bytes() > 0);
    let audit = e.audit.calls.lock().unwrap().clone();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].execution_id, "exec1");
    e.hub.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ts_destructive_call_surfaces_needs_approval_then_succeeds_with_token() {
    let e = env_with(vec![fixture("destructive")], json!({}), |_| {});
    e.hub.sync_catalog().await.unwrap();
    let cm = code_mode(&e).await;
    let code = r#"async function run() { return await Destructive.deleteItem({id:"1"}); }"#;

    let (registry, _) = e.hub.build_registry("e1", vec![]).unwrap();
    let out = run_ts(&cm, code, registry).await;
    assert!(!out.success);
    let payload = CerebroErrorPayload::parse_from(&out.stderr)
        .or_else(|| {
            out.trace.events.iter().find_map(|ev| {
                CerebroErrorPayload::parse_from(
                    &serde_json::to_string(ev).unwrap().replace("\\\"", "\""),
                )
            })
        })
        .unwrap_or_else(|| panic!("sin payload; stderr={}", out.stderr));
    assert_eq!(payload.code, ErrorCode::NeedsApproval);
    let token = payload.approval_token.unwrap();

    let (registry, _) = e.hub.build_registry("e2", vec![token]).unwrap();
    let out = run_ts(&cm, code, registry).await;
    assert!(out.success, "stderr: {}", out.stderr);
    e.hub.shutdown().await;
    let _ = Arc::strong_count(&e.hub);
}
