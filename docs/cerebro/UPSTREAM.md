# UPSTREAM — relación de Cerebro con pctx

> Documento en construcción (WS1). Esta primera versión recoge solo la lista de parches; el resto (API, límites, runbook) se añade en los siguientes commits.

## Parches `CEREBRO-PATCH` aplicados

Todo cambio en `crates/pctx*` está marcado en el código con `// CEREBRO-PATCH: <motivo>`. Búscalos con `git grep CEREBRO-PATCH`. Los parches son **aditivos**: la API pública de pctx se conserva y los valores por defecto son los de upstream, salvo el timeout del type-check (C3).

| ID | Crate | Cambio | Motivo |
|---|---|---|---|
| C1 | `pctx_type_check_runtime`, `pctx_executor` | Límite de heap de V8: `heap_create_params`, `install_heap_guard` (near-heap-limit callback → `terminate_execution` y sube el límite un 50 % / 32 MB para evitar el abort fatal). `ExecuteOptions::heap_limit_bytes` + `with_heap_limit`. Por defecto `None` (V8 por defecto, como upstream). | Sin límite, una bomba de memoria aborta el proceso host (OOM fatal de V8). |
| C2 | `pctx_code_mode` | `CodeMode::execute_typescript_with_options(code, disclosure, registry, ExecuteTypescriptOptions { timeout, heap_limit_bytes })`. `execute_typescript` delega con `Default` (120 s, sin límite de heap). | `CodeMode` no exponía el timeout (`DEFAULT_TIMEOUT` fijo en `code_mode.rs`). |
| C3 | `pctx_type_check_runtime`, `pctx_executor` | `type_check_with_options(code, TypeCheckOptions { timeout, heap_limit_bytes })` con watchdog; nuevas variantes `TypeCheckError::Timeout` y `HeapLimitExceeded`. `ExecuteOptions::type_check_timeout` (por defecto `Some(60 s)`, `DEFAULT_TYPE_CHECK_TIMEOUT`). En `execute`, esos dos errores se devuelven como `ExecuteResult` fallido (stderr: `Type check exceeded …` / `Memory limit of N bytes exceeded …`). | El type-check no tenía timeout y retiene `V8_MUTEX`: un código patológico bloqueaba todo el proceso. |

Cambios de comportamiento respecto a upstream: **(1)** el type-check se corta a los 60 s por defecto; **(2)** se añaden campos públicos a `ExecuteOptions` (rompería solo a quien la construya con un literal de struct; en este repo nadie lo hace) y dos variantes a `TypeCheckError` (rompería un `match` exhaustivo externo; no hay ninguno en el repo).

Cobertura: `crates/cerebro_spike_pctx/tests/spike.rs` (`memory_bomb_is_stopped_with_heap_limit`, `code_mode_options_stop_memory_bomb_and_timeout`, `type_check_timeout_stops_the_check`, `type_check_heap_limit_survives`) y los tests upstream de `pctx_executor`, `pctx_code_mode` y `pctx_type_check_runtime`, que siguen en verde.

**Límite conocido (no cubierto):** el límite de heap de V8 no cubre memoria fuera del heap. Verificado: `new Uint8Array(3 GiB)` **tiene éxito** con `heap_limit = 64 MB`. Queda como riesgo residual; mitigación posible en la Fase 1: límite de memoria del proceso (cgroup/`ulimit -v`) o un pool de procesos worker.
