//! Spike WS1-H3: comportamiento real de pctx que Cerebro da por hecho.
//!
//! Todos los tests usan `#[serial]` porque pctx serializa V8 con un mutex global
//! (`pctx_executor::V8_MUTEX`) y así los tiempos medidos no se contaminan.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cerebro_types::{CerebroErrorPayload, ErrorCode};
use pctx_code_mode::CodeMode;
use pctx_code_mode::config::ToolDisclosure;
use pctx_code_mode::model::{CallbackConfig, GetFunctionDetailsInput};
use pctx_code_mode::registry::{CallbackFn, PctxRegistry};
use serde_json::{Value, json};
use serial_test::serial;

fn cb(namespace: &str, name: &str) -> CallbackConfig {
    CallbackConfig {
        name: name.to_string(),
        namespace: Some(namespace.to_string()),
        description: Some(format!("{name} tool")),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"]
        })),
        output_schema: None,
    }
}

fn echo_fn() -> CallbackFn {
    Arc::new(|args: Option<Value>| {
        Box::pin(async move {
            let text = args
                .as_ref()
                .and_then(|a| a.get("text"))
                .cloned()
                .unwrap_or(Value::Null);
            Ok(json!({ "text": text }))
        })
    })
}

fn failing_fn(payload: CerebroErrorPayload) -> CallbackFn {
    Arc::new(move |_args: Option<Value>| {
        let p = payload.clone();
        Box::pin(async move { Err(p.to_callback_error()) })
    })
}

/// Registry nuevo con un callback eco para cada id de `cm`.
fn echo_registry(cm: &CodeMode) -> PctxRegistry {
    let registry = PctxRegistry::default();
    for c in cm.callbacks() {
        registry.add_callback(&c.id(), echo_fn()).unwrap();
    }
    registry
}

// ───────────────────────── Nombres TS ─────────────────────────

/// Namespace = PascalCase(heck) de `CallbackConfig::namespace`; función = lowerCamelCase(heck)
/// de `name`; id del callback = `"{namespace}__{name}"` con los valores crudos.
#[test]
fn ts_naming_rules() {
    let mut cm = CodeMode::default();
    for (ns, name) in [
        ("echo", "echo"),
        ("github", "create_issue"),
        ("my-server", "get-file.contents"),
        ("my_server2", "List Items"),
        ("UPPER", "HTTPRequest"),
    ] {
        cm.add_callback(&cb(ns, name)).unwrap();
    }
    let listed: Vec<(String, String)> = cm
        .list_functions()
        .functions
        .into_iter()
        .map(|f| (f.namespace, f.name))
        .collect();
    assert_eq!(
        listed,
        vec![
            ("Echo".into(), "echo".into()),
            ("Github".into(), "createIssue".into()),
            ("MyServer".into(), "getFileContents".into()),
            ("MyServer2".into(), "listItems".into()),
            ("Upper".into(), "httpRequest".into()),
        ]
    );
    let ids: Vec<String> = cm.callbacks().iter().map(CallbackConfig::id).collect();
    assert_eq!(
        ids,
        vec![
            "echo__echo",
            "github__create_issue",
            "my-server__get-file.contents",
            "my_server2__List Items",
            "UPPER__HTTPRequest"
        ]
    );

    // get_function_details usa el namespace en PascalCase: "Echo.echo" sí, "echo.echo" no.
    let details: GetFunctionDetailsInput =
        serde_json::from_value(json!({ "functions": ["echo.echo"] })).unwrap();
    assert!(cm.get_function_details(details).functions.is_empty());
    let details: GetFunctionDetailsInput =
        serde_json::from_value(json!({ "functions": ["Echo.echo"] })).unwrap();
    let out = cm.get_function_details(details);
    assert_eq!(out.functions.len(), 1);
    assert!(out.code.contains("namespace Echo"), "{}", out.code);
    assert!(
        out.code.contains("export async function echo("),
        "{}",
        out.code
    );
}

/// Sin namespace, pctx usa `Tools` y el id es solo el nombre.
#[test]
fn ts_naming_without_namespace() {
    let mut cm = CodeMode::default();
    let mut c = cb("x", "do_thing");
    c.namespace = None;
    cm.add_callback(&c).unwrap();
    let f = &cm.list_functions().functions[0];
    assert_eq!(
        (f.namespace.as_str(), f.name.as_str()),
        ("Tools", "doThing")
    );
    assert_eq!(c.id(), "do_thing");
}

/// COLISIONES: pctx solo rechaza nombres crudos repetidos dentro del mismo namespace crudo.
/// Dos nombres crudos distintos que dan el mismo nombre TS **se registran sin error** y la
/// ejecución falla después. Cerebro (WS3) debe detectarlo antes de registrar.
#[tokio::test]
#[serial]
async fn ts_name_collisions_are_not_detected_at_registration() {
    // Mismo namespace crudo, nombre crudo idéntico → error de registro.
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("s", "dup")).unwrap();
    assert!(cm.add_callback(&cb("s", "dup")).is_err());

    // Mismo namespace, `create_issue` y `createIssue` → ambas `createIssue`: se registran.
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("s", "create_issue")).unwrap();
    cm.add_callback(&cb("s", "createIssue")).unwrap();
    let out = cm
        .execute_typescript(
            "async function run() { return await S.createIssue({ text: 'x' }); }",
            ToolDisclosure::Catalog,
            Some(echo_registry(&cm)),
        )
        .await
        .unwrap();
    assert!(!out.success, "se esperaba fallo por función duplicada");
    eprintln!("[colisión de función] stderr = {}", out.stderr);

    // Namespaces crudos distintos (`my-server` y `my_server`) → ambos `MyServer`.
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("my-server", "a")).unwrap();
    cm.add_callback(&cb("my_server", "b")).unwrap();
    let out = cm
        .execute_typescript(
            "async function run() { return [await MyServer.a({ text: '1' }), await MyServer.b({ text: '2' })]; }",
            ToolDisclosure::Catalog,
            Some(echo_registry(&cm)),
        )
        .await
        .unwrap();
    eprintln!(
        "[colisión de namespace] success = {} stderr = {} output = {:?}",
        out.success, out.stderr, out.output
    );
    // TS fusiona `namespace MyServer` repetidos; si las funciones no chocan, funciona.
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.output, Some(json!([{ "text": "1" }, { "text": "2" }])));
}

// ───────────────────────── Callbacks y registry ─────────────────────────

#[tokio::test]
#[serial]
async fn callback_is_called_from_typescript() {
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("echo", "echo")).unwrap();
    let out = cm
        .execute_typescript(
            r#"async function run() {
                 console.log("hola");
                 const r = await Echo.echo({ text: "ping" });
                 return r;
               }"#,
            ToolDisclosure::Catalog,
            Some(echo_registry(&cm)),
        )
        .await
        .unwrap();
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.output, Some(json!({ "text": "ping" })));
    assert_eq!(out.stdout, "hola");
    // La traza recoge la invocación del callback con su id crudo.
    let trace = serde_json::to_value(&out.trace).unwrap();
    eprintln!("[trace] {trace}");
    assert!(trace.to_string().contains("echo__echo"));
}

/// Mismo `CodeMode`, un registry nuevo por ejecución: funciona y no comparten estado.
#[tokio::test]
#[serial]
async fn fresh_registry_per_execution() {
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("echo", "echo")).unwrap();
    let code = "async function run() { return await Echo.echo({ text: 'x' }); }";
    for tag in ["a", "b"] {
        let registry = PctxRegistry::default();
        let tag = tag.to_string();
        registry
            .add_callback(
                "echo__echo",
                Arc::new(move |_| {
                    let tag = tag.clone();
                    Box::pin(async move { Ok(json!(tag)) })
                }),
            )
            .unwrap();
        let out = cm
            .execute_typescript(code, ToolDisclosure::Catalog, Some(registry))
            .await
            .unwrap();
        assert!(out.success, "{}", out.stderr);
        eprintln!("[registry por ejecución] {:?}", out.output);
    }
}

/// Si al registry le falta un id del `CodeMode`, `execute_typescript` devuelve `Err` antes de
/// ejecutar nada (no un `success: false`).
#[tokio::test]
#[serial]
async fn missing_registry_id_is_an_error() {
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("echo", "echo")).unwrap();
    cm.add_callback(&cb("echo", "other")).unwrap();
    let registry = PctxRegistry::default();
    registry.add_callback("echo__echo", echo_fn()).unwrap();
    let err = cm
        .execute_typescript(
            "async function run() { return 1; }",
            ToolDisclosure::Catalog,
            Some(registry),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Registry missing ids"), "{err}");
    assert!(err.to_string().contains("echo__other"), "{err}");
}

// ───────────────────────── Errores de callback ─────────────────────────

/// Un `Err(String)` de callback no capturado llega a `stderr` y `parse_from` lo encuentra.
#[tokio::test]
#[serial]
async fn callback_error_reaches_stderr_and_parses() {
    let payload = CerebroErrorPayload {
        code: ErrorCode::NeedsApproval,
        message: "destructive.deleteItem requires approval".into(),
        approval_token: Some("tok_123".into()),
        suggestions: vec!["retry with approvals".into()],
    };
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("destructive", "delete_item")).unwrap();
    let registry = PctxRegistry::default();
    registry
        .add_callback("destructive__delete_item", failing_fn(payload.clone()))
        .unwrap();
    let out = cm
        .execute_typescript(
            "async function run() { return await Destructive.deleteItem({ text: '1' }); }",
            ToolDisclosure::Catalog,
            Some(registry),
        )
        .await
        .unwrap();
    assert!(!out.success);
    eprintln!("[stderr error callback] {}", out.stderr);
    assert_eq!(CerebroErrorPayload::parse_from(&out.stderr), Some(payload));
}

/// Si el código TS captura el error, `e.message` también contiene el payload parseable.
#[tokio::test]
#[serial]
async fn callback_error_is_catchable_in_typescript() {
    let payload = CerebroErrorPayload {
        code: ErrorCode::Denied,
        message: "denied by policy".into(),
        approval_token: None,
        suggestions: vec![],
    };
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("s", "t")).unwrap();
    let registry = PctxRegistry::default();
    registry
        .add_callback("s__t", failing_fn(payload.clone()))
        .unwrap();
    let out = cm
        .execute_typescript(
            "async function run() { try { await S.t({ text: '' }); return 'no'; } catch (e) { return String((e as Error).message); } }",
            ToolDisclosure::Catalog,
            Some(registry),
        )
        .await
        .unwrap();
    assert!(out.success, "{}", out.stderr);
    let msg = out.output.unwrap();
    eprintln!("[e.message] {msg}");
    assert_eq!(
        CerebroErrorPayload::parse_from(msg.as_str().unwrap()),
        Some(payload)
    );
}

/// Documenta un hueco de pctx: si el script escribió algo con `console.error` antes de
/// lanzar, `stderr` contiene **solo** la consola y el mensaje del error se pierde
/// (`pctx_executor/src/lib.rs:239-243`). WS4 no debe depender solo de `stderr`.
#[tokio::test]
#[serial]
async fn console_error_hides_uncaught_error_message() {
    let payload = CerebroErrorPayload {
        code: ErrorCode::ToolError,
        message: "boom".into(),
        approval_token: None,
        suggestions: vec![],
    };
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("s", "t")).unwrap();
    let registry = PctxRegistry::default();
    registry
        .add_callback("s__t", failing_fn(payload.clone()))
        .unwrap();
    let out = cm
        .execute_typescript(
            "async function run() { console.error('aviso'); return await S.t({ text: '' }); }",
            ToolDisclosure::Catalog,
            Some(registry),
        )
        .await
        .unwrap();
    assert!(!out.success);
    assert_eq!(out.stderr, "aviso");
    assert_eq!(CerebroErrorPayload::parse_from(&out.stderr), None);
    // El payload sí queda en la traza del registry (evento CallbackInvocation con outcome Error).
    let trace = serde_json::to_string(&out.trace).unwrap();
    let from_trace = CerebroErrorPayload::parse_from(&trace.replace("\\\"", "\""));
    assert_eq!(from_trace, Some(payload));
}

// ───────────────────────── Límites del sandbox ─────────────────────────

/// Un bucle infinito acaba por el watchdog. `CodeMode::execute_typescript` no expone el
/// timeout (siempre 120 s): aquí se usa `pctx_executor::execute` con `with_timeout`.
#[tokio::test]
#[serial]
async fn infinite_loop_is_stopped_by_timeout() {
    let start = Instant::now();
    let res = pctx_executor::execute(
        "async function run() { while (true) {} }\nexport default await run();",
        pctx_executor::ExecuteOptions::new().with_timeout(Duration::from_secs(2)),
    )
    .await
    .unwrap();
    let elapsed = start.elapsed();
    eprintln!("[timeout] {:?} stderr = {}", elapsed, res.stderr);
    assert!(!res.success);
    assert!(res.stderr.contains("exceeded 2s"), "{}", res.stderr);
    assert!(elapsed < Duration::from_secs(10));

    // Una promesa que nunca se resuelve también corta (temporizador de tokio).
    let res = pctx_executor::execute(
        "async function run() { await new Promise(() => {}); return 1; }\nexport default await run();",
        pctx_executor::ExecuteOptions::new().with_timeout(Duration::from_secs(1)),
    )
    .await
    .unwrap();
    assert!(!res.success);
    assert!(res.stderr.contains("exceeded 1s"), "{}", res.stderr);

    // Tras un timeout, el proceso sigue sirviendo ejecuciones.
    let res = pctx_executor::execute("async function run() { return 2; }\nexport default await run();", pctx_executor::ExecuteOptions::new())
        .await
        .unwrap();
    assert_eq!(res.output, Some(json!(2)));
}

/// Cero red: el runtime no incluye `deno_fetch`/`deno_net`, así que `fetch`, `WebSocket` y
/// `Deno.connect` no existen. Además `import()` dinámico se rechaza (NoopModuleLoader).
#[tokio::test]
#[serial]
async fn sandbox_has_no_network_nor_fs() {
    let probe = r#"
        async function run() {
        const r: Record<string, string> = {};
        const g = globalThis as any;
        r.fetch = typeof g.fetch;
        r.WebSocket = typeof g.WebSocket;
        r.XMLHttpRequest = typeof g.XMLHttpRequest;
        r.DenoConnect = typeof g.Deno?.connect;
        r.DenoReadFile = typeof g.Deno?.readTextFile;
        r.DenoOpen = typeof g.Deno?.open;
        r.ops = Object.keys(g.Deno?.core?.ops ?? {}).filter((k) => /net|fetch|fs|read|write|open|connect|http|tcp|dns|env|spawn|run/i.test(k)).join(",");
        try { await g.fetch("https://example.com"); r.fetchCall = "ok"; } catch (e) { r.fetchCall = "error: " + String(e); }
        try { await (new Function("u", "return import(u)") as any)("file:///etc/passwd"); r.importFs = "ok"; } catch (e) { r.importFs = "error: " + String(e); }
        try { await (new Function("u", "return import(u)") as any)("https://example.com/x.js"); r.importNet = "ok"; } catch (e) { r.importNet = "error: " + String(e); }
        return r;
        }
        export default await run();
    "#;
    let res = pctx_executor::execute(probe, pctx_executor::ExecuteOptions::new())
        .await
        .unwrap();
    assert!(res.success, "{}", res.stderr);
    let r = res.output.unwrap();
    eprintln!("[sandbox] {}", serde_json::to_string_pretty(&r).unwrap());
    assert_eq!(r["fetch"], "undefined");
    assert_eq!(r["WebSocket"], "undefined");
    assert_eq!(r["XMLHttpRequest"], "undefined");
    assert_eq!(r["DenoConnect"], "undefined");
    assert_eq!(r["DenoReadFile"], "undefined");
    assert_eq!(r["DenoOpen"], "undefined");
    assert!(r["fetchCall"].as_str().unwrap().starts_with("error"));
    assert!(r["importFs"].as_str().unwrap().starts_with("error"));
    assert!(r["importNet"].as_str().unwrap().starts_with("error"));
}

/// Desde `execute_typescript` (con type-check) `fetch(...)` ni siquiera compila: el lib de TS
/// lo declara, pero en runtime no existe. Se documenta qué pasa.
#[tokio::test]
#[serial]
async fn fetch_via_code_mode_fails() {
    let cm = CodeMode::default();
    let out = cm
        .execute_typescript(
            r#"async function run() { const r = await fetch("https://example.com"); return r.status; }"#,
            ToolDisclosure::Catalog,
            Some(PctxRegistry::default()),
        )
        .await
        .unwrap();
    eprintln!(
        "[fetch code mode] success={} stderr={}",
        out.success, out.stderr
    );
    assert!(!out.success);
    assert!(out.output.is_none());
}

/// Type-check: siempre se ejecuta antes. Mide la primera ejecución y las siguientes.
#[tokio::test]
#[serial]
async fn type_check_always_runs_and_timing() {
    let mut cm = CodeMode::default();
    cm.add_callback(&cb("echo", "echo")).unwrap();

    // Error de tipos → no se ejecuta el callback, success=false, stderr con "TS2345"/línea.
    let registry = echo_registry(&cm);
    let out = cm
        .execute_typescript(
            "async function run() { return await Echo.echo({ text: 42 }); }",
            ToolDisclosure::Catalog,
            Some(registry.clone()),
        )
        .await
        .unwrap();
    eprintln!("[type error] {}", out.stderr);
    assert!(!out.success);
    assert!(out.stderr.contains("TS"), "{}", out.stderr);
    assert!(
        registry.trace().events().is_empty(),
        "no debe llamar al callback"
    );

    let code = "async function run() { return await Echo.echo({ text: 'x' }); }";
    let mut times = vec![];
    for _ in 0..4 {
        let t = Instant::now();
        let out = cm
            .execute_typescript(code, ToolDisclosure::Catalog, Some(echo_registry(&cm)))
            .await
            .unwrap();
        assert!(out.success, "{}", out.stderr);
        let tc = out
            .trace
            .events
            .iter()
            .find_map(|e| match e {
                pctx_code_mode::model::ExecutionEvent::TypeCheck(tc) => {
                    tc.ended_at.duration_since(tc.started_at).ok()
                }
                _ => None,
            })
            .unwrap();
        times.push((t.elapsed(), tc));
    }
    eprintln!("[timing] (total, type_check) por ejecución: {times:?}");
}

// ───────────────────────── Memoria ─────────────────────────

/// Proceso hijo: bomba de memoria con el heap de V8 limitado por flag a 64 MB para que sea
/// rápido. Solo corre cuando lo lanza `memory_bomb_kills_the_host_process`.
#[tokio::test]
#[ignore = "proceso hijo de memory_bomb_kills_the_host_process; no se ejecuta solo"]
async fn memory_bomb_child() {
    deno_core::v8_set_flags(vec!["".into(), "--max-old-space-size=64".into()]);
    let res = pctx_executor::execute(
        "const a: string[] = []; while (true) { a.push('x'.repeat(1024) + a.length); }\nexport default 1;",
        pctx_executor::ExecuteOptions::new().with_timeout(Duration::from_secs(60)),
    )
    .await;
    // Si llegamos aquí, V8 no abortó el proceso.
    println!("CHILD_SURVIVED {res:?}", res = res.map(|r| r.stderr));
}

/// SIN parche, pctx no fija límite de heap ni `near_heap_limit_callback`: al agotar el heap,
/// V8 aborta **todo el proceso** (OOM fatal), no lanza una excepción JS. Se verifica en un
/// proceso hijo para no tumbar el runner de tests. Cuando se aplique el parche
/// `CEREBRO-PATCH` de memoria, este test debe invertirse (el hijo sobrevive).
#[test]
fn memory_bomb_kills_the_host_process() {
    let exe = std::env::current_exe().unwrap();
    let start = Instant::now();
    let out = std::process::Command::new(exe)
        .args(["memory_bomb_child", "--exact", "--ignored", "--nocapture"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    eprintln!(
        "[memory bomb] {:?} status={:?}\nstdout tail: {}\nstderr tail: {}",
        start.elapsed(),
        out.status,
        tail(&stdout),
        tail(&stderr)
    );
    assert!(
        !stdout.contains("CHILD_SURVIVED"),
        "el hijo sobrevivió: ¿hay ya límite de memoria?"
    );
    assert!(!out.status.success());
}

fn tail(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(6)..].join("\n")
}
