#![allow(clippy::pedantic)]
mod common;

use cerebro_types::*;
use common::*;
use serde_json::json;

fn two() -> Env {
    env_with(
        vec![fixture("echo"), fixture("destructive")],
        json!({}),
        |_| {},
    )
}

#[tokio::test]
async fn sync_catalogs_both_fixtures() {
    let e = two();
    let r = e.hub.sync_catalog().await.unwrap();
    assert_eq!(r.servers_ok, vec!["destructive", "echo"]);
    assert!(r.servers_failed.is_empty(), "{:?}", r.servers_failed);
    assert_eq!(r.tools, 4);
    let echo = e.catalog.get_tool("echo", "echo").unwrap().unwrap();
    assert_eq!(echo.ts_name, "echo");
    assert_eq!(echo.read_only_hint(), Some(true));
    assert_eq!(echo.schema_hash.len(), 64);
    assert!(echo.approx_tokens > 0);
    let srv = e.catalog.list_servers().unwrap();
    let echo_srv = srv.iter().find(|s| s.name == "echo").unwrap();
    assert_eq!(echo_srv.ts_namespace, "Echo");
    assert_eq!(echo_srv.status, ServerStatus::Ok);
    assert!(echo_srv.catalog_hash.is_some());
    // Idempotente: el hash de esquema no cambia entre sincronizaciones.
    e.hub.sync_catalog().await.unwrap();
    assert_eq!(
        e.catalog
            .get_tool("echo", "echo")
            .unwrap()
            .unwrap()
            .schema_hash,
        echo.schema_hash
    );
    e.hub.shutdown().await;
}

#[tokio::test]
async fn call_echo_is_ok_and_audited_with_redaction() {
    let e = two();
    e.hub.sync_catalog().await.unwrap();
    let v = e
        .hub
        .call(
            "echo",
            "echo",
            Some(json!({"text": "hola"})),
            &ctx("x1", &[]),
        )
        .await
        .unwrap();
    assert_eq!(v, json!({"text": "hola"}));
    let calls = e.audit.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].decision, CallDecision::Allowed);
    assert_eq!(calls[0].status, CallStatus::Ok);
    assert_eq!(calls[0].execution_id, "x1");
    assert!(calls[0].result_bytes > 0);
    // Argumentos con secretos se redactan en la auditoría.
    let _ = e
        .hub
        .call(
            "echo",
            "echo",
            Some(json!({"text": "a", "apiToken": "s3cr3t"})),
            &ctx("x2", &[]),
        )
        .await;
    let last = e.audit.calls.lock().unwrap().last().unwrap().clone();
    assert_eq!(last.args_redacted["apiToken"], "***");
    assert_eq!(last.args_redacted["text"], "a");
    e.hub.shutdown().await;
}

#[tokio::test]
async fn destructive_needs_approval_then_token_is_single_use() {
    let e = two();
    e.hub.sync_catalog().await.unwrap();
    let args = json!({"id": "1"});
    let err = e
        .hub
        .call(
            "destructive",
            "delete_item",
            Some(args.clone()),
            &ctx("x", &[]),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NeedsApproval);
    let token = err.approval_token.expect("token");
    assert_eq!(
        e.audit.calls.lock().unwrap().last().unwrap().decision,
        CallDecision::NeedsApproval
    );

    // Con el token, ok.
    let ok = e
        .hub
        .call(
            "destructive",
            "delete_item",
            Some(args.clone()),
            &ctx("x", &[&token]),
        )
        .await
        .unwrap();
    assert_eq!(
        ok["text"].as_str().map(|t| t.contains("deleted")),
        Some(true)
    );
    assert_eq!(
        e.audit.calls.lock().unwrap().last().unwrap().decision,
        CallDecision::Confirmed
    );

    // El mismo token otra vez: ya gastado → NEEDS_APPROVAL con token nuevo.
    let err2 = e
        .hub
        .call(
            "destructive",
            "delete_item",
            Some(args),
            &ctx("x", &[&token]),
        )
        .await
        .unwrap_err();
    assert_eq!(err2.code, ErrorCode::NeedsApproval);
    assert_ne!(err2.approval_token.as_deref(), Some(token.as_str()));

    // Un token para otros argumentos no sirve.
    let err3 = e
        .hub
        .call(
            "destructive",
            "delete_item",
            Some(json!({"id": "2"})),
            &ctx("x", &[err2.approval_token.as_deref().unwrap()]),
        )
        .await
        .unwrap_err();
    assert_eq!(err3.code, ErrorCode::NeedsApproval);
    e.hub.shutdown().await;
}

#[tokio::test]
async fn tool_is_error_maps_to_tool_error() {
    let e = env_with(
        vec![fixture("destructive")],
        json!({"overrides": {"destructive.*": "allow"}}),
        |_| {},
    );
    e.hub.sync_catalog().await.unwrap();
    let err = e
        .hub
        .call(
            "destructive",
            "delete_item",
            Some(json!({"id": "999"})),
            &ctx("x", &[]),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ToolError);
    assert!(err.message.contains("not found"), "{}", err.message);
    assert_eq!(
        e.audit.calls.lock().unwrap().last().unwrap().status,
        CallStatus::Error
    );
    // structuredContent cuando existe.
    let v = e
        .hub
        .call(
            "destructive",
            "read_item",
            Some(json!({"id": "1"})),
            &ctx("x", &[]),
        )
        .await
        .unwrap();
    assert_eq!(v, json!({"id": "1", "name": "alpha", "exists": true}));
    e.hub.shutdown().await;
}

#[tokio::test]
async fn deny_override_blocks_without_running() {
    let e = env_with(
        vec![fixture("echo")],
        json!({"overrides": {"echo.echo": "deny"}}),
        |_| {},
    );
    e.hub.sync_catalog().await.unwrap();
    let err = e
        .hub
        .call("echo", "echo", Some(json!({"text": "x"})), &ctx("x", &[]))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Denied);
    assert_eq!(
        e.audit.calls.lock().unwrap().last().unwrap().status,
        CallStatus::NotRun
    );
    assert_eq!(e.hub.live_servers().await, 1); // el sync lo arrancó; la llamada no hizo nada más
    e.hub.shutdown().await;
}

#[tokio::test]
async fn unknown_tool_has_suggestions() {
    let e = two();
    e.hub.sync_catalog().await.unwrap();
    let err = e
        .hub
        .call("echo", "ech", None, &ctx("x", &[]))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ToolNotFound);
    assert!(
        err.suggestions.contains(&"echo.echo".to_string()),
        "{:?}",
        err.suggestions
    );
    let err = e
        .hub
        .call("nope", "echo", None, &ctx("x", &[]))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ToolNotFound);
    e.hub.shutdown().await;
}

#[tokio::test]
async fn failing_server_does_not_break_the_rest() {
    let mut bad = fixture("echo");
    bad.name = "broken".into();
    bad.command = "/nonexistent/binary".into();
    let e = env_with(vec![bad, fixture("echo")], json!({}), |_| {});
    let r = e.hub.sync_catalog().await.unwrap();
    assert_eq!(r.servers_ok, vec!["echo"]);
    assert_eq!(r.servers_failed.len(), 1);
    assert_eq!(r.servers_failed[0].0, "broken");
    let s = e.catalog.list_servers().unwrap();
    let broken = s.iter().find(|s| s.name == "broken").unwrap();
    assert_eq!(broken.status, ServerStatus::Error);
    assert!(broken.last_error.is_some());
    assert!(
        e.hub
            .call("echo", "echo", Some(json!({"text": "ok"})), &ctx("x", &[]))
            .await
            .is_ok()
    );
    // Un fallo posterior en un servidor ya catalogado actualiza su estado, sin borrar herramientas.
    e.hub.shutdown().await;
}

#[tokio::test]
async fn ts_name_collisions_keep_first_and_warn() {
    let e = env_with(vec![script("my-server", "collide.mjs")], json!({}), |_| {});
    let r = e.hub.sync_catalog().await.unwrap();
    assert_eq!(r.servers_ok, vec!["my-server"]);
    assert_eq!(r.tools, 2, "createIssue + other");
    assert_eq!(r.warnings.len(), 1, "{:?}", r.warnings);
    assert!(r.warnings[0].contains("create_issue") && r.warnings[0].contains("createIssue"));
    // Orden por nombre crudo: `createIssue` < `create_issue` (mayúscula 'I' < '_')? Gana el primero.
    let names: Vec<String> = e
        .catalog
        .list_tools(None)
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert!(
        names.contains(&"createIssue".to_string()) ^ names.contains(&"create_issue".to_string())
    );
    let srv = e.catalog.list_servers().unwrap();
    assert_eq!(srv[0].ts_namespace, "MyServer");
    e.hub.shutdown().await;
}

#[tokio::test]
async fn servers_with_same_ts_namespace_are_rejected() {
    let mut a = fixture("echo");
    a.name = "my-server".into();
    let mut b = fixture("echo");
    b.name = "my_server".into();
    let e = env_with(vec![a, b], json!({}), |_| {});
    let r = e.hub.sync_catalog().await.unwrap();
    assert_eq!(r.servers_ok, vec!["my-server"]);
    assert_eq!(r.servers_failed.len(), 1);
    assert_eq!(r.servers_failed[0].0, "my_server");
    e.hub.shutdown().await;
}

#[tokio::test]
async fn lazy_start_and_idle_shutdown_restart() {
    let e = env_with(vec![fixture("echo")], json!({}), |c| {
        c.idle_shutdown = std::time::Duration::from_millis(300)
    });
    e.hub.sync_catalog().await.unwrap();
    assert_eq!(e.hub.live_servers().await, 1);
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    assert_eq!(
        e.hub.live_servers().await,
        0,
        "el reaper debe apagar el servidor inactivo"
    );
    // Arranque perezoso al siguiente uso.
    let v = e
        .hub
        .call(
            "echo",
            "echo",
            Some(json!({"text": "again"})),
            &ctx("x", &[]),
        )
        .await
        .unwrap();
    assert_eq!(v, json!({"text": "again"}));
    assert_eq!(e.hub.live_servers().await, 1);
    e.hub.shutdown().await;
    assert_eq!(e.hub.live_servers().await, 0);
}

#[tokio::test]
async fn concurrent_calls_share_one_process() {
    let e = env_with(vec![fixture("echo")], json!({}), |_| {});
    // Sin sync previo: las llamadas concurrentes compiten por el arranque perezoso.
    e.hub.sync_catalog().await.unwrap();
    e.hub.shutdown().await;
    let h = e.hub.clone();
    let rs = futures_join((0..8).map(|i| {
        let h = h.clone();
        async move {
            h.call(
                "echo",
                "echo",
                Some(json!({"text": format!("m{i}")})),
                &ctx("c", &[]),
            )
            .await
        }
    }))
    .await;
    assert!(rs.iter().all(|r| r.is_ok()), "{rs:?}");
    assert_eq!(e.hub.live_servers().await, 1);
    e.hub.shutdown().await;
}

async fn futures_join<F: std::future::Future + Send + 'static>(
    it: impl Iterator<Item = F>,
) -> Vec<F::Output>
where
    F::Output: Send + 'static,
{
    let hs: Vec<_> = it.map(tokio::spawn).collect();
    let mut out = vec![];
    for h in hs {
        out.push(h.await.unwrap());
    }
    out
}
