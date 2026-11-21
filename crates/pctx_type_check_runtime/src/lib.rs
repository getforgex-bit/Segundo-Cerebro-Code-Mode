//! # PCTX Type Check
//!
//! An isolated TypeScript type checking runtime powered by Deno and the official TypeScript compiler.
//!
//! ## Overview
//!
//! This crate provides a sandboxed environment for performing full semantic type checking on
//! TypeScript code. It embeds the TypeScript 5.3.3 compiler in a V8 snapshot for fast startup
//! and uses a separate Deno runtime instance to ensure complete isolation.
//!
//! ## Features
//!
//! - **Full Semantic Analysis**: Uses the official TypeScript compiler for complete type checking
//! - **Isolated Runtime**: Each type check runs in its own sandboxed Deno runtime
//! - **Fast Startup**: TypeScript compiler embedded in V8 snapshot (~20s build time, instant runtime)
//! - **JavaScript Compatible**: Filters TypeScript-only errors to allow valid JavaScript code
//! - **Async Support**: Provides both sync and async APIs
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use pctx_type_check_runtime::type_check;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Type check TypeScript code
//! let code = r#"
//!     const x: number = 42;
//!     const y: string = "hello";
//!     export default x + y.length;
//! "#;
//!
//! let result = type_check(code)?;
//! if result.success {
//!     println!("Type check passed!");
//! } else {
//!     for diagnostic in result.diagnostics {
//!         println!("{}: {}", diagnostic.severity, diagnostic.message);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Filtering
//!
//! The type checker automatically filters out errors that don't affect runtime execution,
//! such as:
//! - Missing console/Promise declarations
//! - Implicit `any` types
//! - Dynamic object access
//! - Unknown exception types
//!
//! This allows JavaScript code to type-check successfully while still catching real type errors.
//!
//! ## Performance
//!
//! - **Build Time**: ~20 seconds (one-time cost to create V8 snapshot with TypeScript compiler)
//! - **Runtime**: ~40-60ms per type check for typical code
//! - **Memory**: Isolated runtime per check, cleaned up automatically
//!
//! ## Snapshot Details
//!
//! The crate exports a pre-compiled V8 snapshot that includes:
//! - TypeScript 5.3.3 compiler (9MB source)
//! - Type checking runtime logic
//! - Minimal lib.d.ts definitions for common types
//!
//! The snapshot is embedded at compile time and accessible via [`TYPE_CHECK_SNAPSHOT`].

#![allow(long_running_const_eval)]

pub mod ignored_codes;

use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;
use thiserror::Error;

/// Result type alias for type checking operations
pub type Result<T> = std::result::Result<T, TypeCheckError>;

/// Errors that can occur during type checking
#[derive(Debug, Error)]
pub enum TypeCheckError {
    /// Internal error in the type checking runtime
    #[error("Internal type check error: {0}")]
    InternalError(String),

    /// Error parsing the TypeScript code
    #[error("Parse error: {0}")]
    ParseError(String),

    // CEREBRO-PATCH: el type-check ahora puede cortarse por tiempo o por heap en vez de colgar o abortar el host.
    /// The type check ran longer than the allowed time and was stopped.
    #[error("Type check exceeded {0:?} and was stopped")]
    Timeout(Duration),

    // CEREBRO-PATCH: ver arriba.
    /// The V8 heap reached its limit and the isolate was stopped.
    #[error("Memory limit of {0} bytes exceeded and execution was stopped")]
    HeapLimitExceeded(usize),
}

// CEREBRO-PATCH: límite de heap de V8 compartido por el type-check y el executor.
// Sin él, una bomba de memoria hace abortar (OOM fatal) todo el proceso host.
/// Build V8 `CreateParams` with a hard heap limit. `None` keeps V8's defaults.
pub fn heap_create_params(limit_bytes: Option<usize>) -> Option<deno_core::v8::CreateParams> {
    // CEREBRO-PATCH: ver arriba.
    limit_bytes.map(|max| deno_core::v8::CreateParams::default().heap_limits(0, max))
}

/// Install a near-heap-limit callback on `js_runtime`: when V8 gets close to the limit it
/// sets `exceeded`, terminates the script and raises the limit a bit so V8 does not abort
/// the whole process while the termination propagates.
pub fn install_heap_guard(js_runtime: &mut JsRuntime, exceeded: Arc<AtomicBool>) {
    // CEREBRO-PATCH: ver arriba.
    let handle = js_runtime.v8_isolate().thread_safe_handle();
    js_runtime.add_near_heap_limit_callback(move |current, _initial| {
        exceeded.store(true, Ordering::SeqCst);
        handle.terminate_execution();
        current + (current / 2).max(32 * 1024 * 1024)
    });
}

// CEREBRO-PATCH: opciones del type-check (tiempo y heap).
/// Limits for a single type check. The default (`None`/`None`) matches upstream behavior.
#[derive(Debug, Clone, Copy, Default)]
pub struct TypeCheckOptions {
    /// Stop the type check after this long.
    pub timeout: Option<Duration>,
    /// Hard V8 heap limit in bytes.
    pub heap_limit_bytes: Option<usize>,
}

/// A single type checking diagnostic (error or warning)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    /// Human-readable error message
    pub message: String,
    /// Line number where the error occurred (1-indexed)
    pub line: Option<usize>,
    /// Column number where the error occurred (1-indexed)
    pub column: Option<usize>,
    /// Severity level: "error" or "warning"
    pub severity: String,
    /// TypeScript diagnostic code (e.g., 2322 for type mismatch)
    pub code: Option<u32>,
}

/// Result of a type checking operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckResult {
    /// Whether the code passed type checking (no errors)
    pub success: bool,
    /// List of diagnostics found during type checking
    pub diagnostics: Vec<Diagnostic>,
}

/// Pre-compiled V8 snapshot containing the TypeScript compiler
///
/// This snapshot is created at build time and includes:
/// - TypeScript 5.3.3 compiler (full semantic analysis)
/// - Type checking runtime with lib.d.ts definitions
/// - MCP SDK type definitions
///
/// The snapshot is ~10MB and loads instantly at runtime.
pub static TYPE_CHECK_SNAPSHOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/PCTX_TYPE_CHECK_SNAPSHOT.bin"));

/// TypeScript lib.d.ts files for ES2020 support
///
/// This JSON contains all TypeScript standard library definition files,
/// providing type definitions for built-in JavaScript types (Array, Promise,
/// Map, console, etc.). These are injected into the type checker runtime
/// to enable full ES2020 type checking.
static TS_LIBS_JSON: &str = include_str!("../ts-libs.json");

// Define the type check extension
deno_core::extension!(
    pctx_type_check_snapshot,
    esm_entry_point = "ext:pctx_type_check_snapshot/type_check_runtime_generated.js",
    esm = [ dir "src", "type_check_runtime_generated.js" ],
);

/// Initialize the V8 platform. Must be called before any `JsRuntime` is created.
/// Safe to call multiple times - only the first call has effect.
static V8_INIT: std::sync::Once = std::sync::Once::new();

/// Ensure V8 platform is initialized. Called automatically by `type_check`.
pub fn init_v8_platform() {
    V8_INIT.call_once(|| {
        deno_core::JsRuntime::init_platform(None);
    });
}

/// Type check TypeScript code using an isolated Deno runtime with TypeScript compiler
///
/// This creates a separate Deno runtime with the TypeScript compiler snapshot to perform
/// full semantic type checking in a sandboxed environment.
///
/// # Arguments
///
/// * `code` - The TypeScript code to type check
///
/// # Returns
///
/// Returns a [`CheckResult`] containing whether the check passed and any diagnostics.
///
/// # Errors
///
/// Returns [`TypeCheckError::ParseError`] if the code has syntax errors.
/// Returns [`TypeCheckError::InternalError`] if the type checking runtime fails.
///
/// # Example
///
/// ```rust,no_run
/// use pctx_type_check_runtime::type_check;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let code = r#"
///     const x: number = "string"; // Type error!
/// "#;
///
/// let result = type_check(code)?;
/// assert!(!result.success);
/// assert_eq!(result.diagnostics.len(), 1);
/// # Ok(())
/// # }
/// ```
pub fn type_check(code: &str) -> Result<CheckResult> {
    type_check_with_options(code, TypeCheckOptions::default())
}

/// Same as [`type_check`] but with a timeout and/or heap limit.
///
/// # Errors
///
/// In addition to [`type_check`]'s errors, returns [`TypeCheckError::Timeout`] or
/// [`TypeCheckError::HeapLimitExceeded`] when a limit is hit.
// CEREBRO-PATCH: versión con límites; `type_check` delega aquí con los valores por defecto.
pub fn type_check_with_options(code: &str, options: TypeCheckOptions) -> Result<CheckResult> {
    // First do a quick syntax check with deno_ast
    let parse_result = deno_ast::parse_module(deno_ast::ParseParams {
        specifier: deno_ast::ModuleSpecifier::parse("file:///check.ts")
            .map_err(|e| TypeCheckError::InternalError(e.to_string()))?,
        text: code.into(),
        media_type: deno_ast::MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    });

    // If syntax parsing fails, return immediately
    if let Err(diagnostic) = parse_result {
        return Ok(CheckResult {
            success: false,
            diagnostics: vec![Diagnostic {
                message: diagnostic.to_string(),
                line: None,
                column: None,
                severity: "error".to_string(),
                code: None,
            }],
        });
    }

    // Ensure V8 platform is initialized (safe to call multiple times)
    init_v8_platform();

    // The type checker never loads modules; refuse all of them rather than reading the host filesystem.
    let mut js_runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(deno_core::NoopModuleLoader)),
        startup_snapshot: Some(TYPE_CHECK_SNAPSHOT),
        extensions: vec![pctx_type_check_snapshot::init()],
        create_params: heap_create_params(options.heap_limit_bytes), // CEREBRO-PATCH: límite de heap
        ..Default::default()
    });

    // CEREBRO-PATCH: guardia de heap + watchdog de timeout para el type-check.
    let heap_exceeded = Arc::new(AtomicBool::new(false));
    if options.heap_limit_bytes.is_some() {
        install_heap_guard(&mut js_runtime, heap_exceeded.clone());
    }
    let timed_out = Arc::new(AtomicBool::new(false));
    let (finished, finished_rx) = channel::<()>();
    let watchdog = options.timeout.map(|timeout| {
        let isolate = js_runtime.v8_isolate().thread_safe_handle();
        let timed_out = timed_out.clone();
        std::thread::spawn(move || {
            if finished_rx.recv_timeout(timeout) == Err(RecvTimeoutError::Timeout) {
                timed_out.store(true, Ordering::SeqCst);
                isolate.terminate_execution();
            }
        })
    });
    let stop_watchdog = move || {
        drop(finished);
        if let Some(w) = watchdog {
            let _ = w.join();
        }
    };
    let classify = |e: String| -> TypeCheckError {
        if heap_exceeded.load(Ordering::SeqCst) {
            TypeCheckError::HeapLimitExceeded(options.heap_limit_bytes.unwrap_or(0))
        } else if timed_out.load(Ordering::SeqCst) {
            TypeCheckError::Timeout(options.timeout.unwrap_or_default())
        } else {
            TypeCheckError::InternalError(e)
        }
    };

    // Inject TypeScript lib files as a global variable
    let inject_libs_script = format!("globalThis.TS_LIBS = {};", TS_LIBS_JSON);
    if let Err(e) = js_runtime.execute_script("<inject_ts_libs>", inject_libs_script) {
        stop_watchdog();
        return Err(classify(format!("Failed to inject TS_LIBS: {}", e)));
    }

    // Call the type checking function from the runtime
    let code_json =
        serde_json::to_string(code).map_err(|e| TypeCheckError::InternalError(e.to_string()))?;

    let check_script = format!(
        r"
        (function() {{
            const code = {code_json};
            return globalThis.typeCheckCode(code);
        }})()
        "
    );

    let result = match js_runtime.execute_script("<type_check>", check_script) {
        Ok(r) => r,
        Err(e) => {
            stop_watchdog();
            return Err(classify(e.to_string()));
        }
    };
    stop_watchdog();

    // Extract the result using v8 scope
    let check_result = {
        deno_core::scope!(scope, &mut js_runtime);
        let local = deno_core::v8::Local::new(scope, result);
        deno_core::serde_v8::from_v8::<CheckResult>(scope, local)
            .map_err(|e| TypeCheckError::InternalError(e.to_string()))?
    };

    Ok(check_result)
}

/// Filters diagnostics to only include errors that indicate runtime failures
///
/// This function determines whether a TypeScript diagnostic represents a real problem that
/// would cause the code to fail at runtime, versus TypeScript-only strictness errors.
///
/// # Filtered Error Codes
///
/// With ES2020 lib files, the following TypeScript error codes are considered irrelevant:
/// - `2307`: Cannot find module (module resolution handled by runtime)
/// - `2304`: Cannot find name 'require' (not used in ESM)
/// - `7016`: Could not find declaration file (not needed for runtime)
/// - `7006`, `7053`, `7005`, `7034`: Implicit any types (JavaScript compatibility)
/// - `2362`, `2363`: Arithmetic operation strictness (runtime handles coercion)
///
/// # Arguments
///
/// * `diagnostic` - The diagnostic to check
///
/// # Returns
///
/// Returns `true` if the error is relevant (would cause runtime failure), `false` otherwise.
///
/// # Example
///
/// ```rust
/// use pctx_type_check_runtime::{Diagnostic, is_relevant_error};
///
/// // Type mismatch - relevant error
/// let type_error = Diagnostic {
///     message: "Type 'string' is not assignable to type 'number'.".to_string(),
///     line: Some(1),
///     column: Some(1),
///     severity: "error".to_string(),
///     code: Some(2322),
/// };
/// assert!(is_relevant_error(&type_error));
///
/// // Module not found - irrelevant (module resolution handled by runtime)
/// let module_error = Diagnostic {
///     message: "Cannot find module './foo'.".to_string(),
///     line: Some(1),
///     column: Some(1),
///     severity: "error".to_string(),
///     code: Some(2307),
/// };
/// assert!(!is_relevant_error(&module_error));
/// ```
pub fn is_relevant_error(diagnostic: &Diagnostic) -> bool {
    // Use the shared ignored codes list from ignored_codes module
    // This list is synchronized with the JavaScript runtime at build time
    match diagnostic.code {
        Some(code) => !ignored_codes::IGNORED_DIAGNOSTIC_CODES.contains(&code),
        None => true, // No code is present is an error
    }
}

/// Returns the crate version
///
/// # Example
///
/// ```rust
/// use pctx_type_check_runtime::version;
///
/// println!("pctx_type_check version: {}", version());
/// ```
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
