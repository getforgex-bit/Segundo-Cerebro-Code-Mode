# UPSTREAM — relación de Cerebro con pctx

Base del fork: `portofcontext/pctx` **v1.0.0** (`upstream/main` @ `499379d`). Todas las referencias `archivo:línea` son del árbol actual del fork; las que cambiaron por nuestros parches se indican como "(upstream)". Lo verificado con tests lleva el nombre del test en `crates/cerebro_spike_pctx/tests/spike.rs`. Lo no verificado se marca **no verificado**.

## 1. API de `CodeMode` (`crates/pctx_code_mode/src/code_mode.rs`)

| Método | Línea | Notas |
|---|---|---|
| `add_callback(&mut self, &CallbackConfig) -> Result<Vec<String>>` | 241 | Crea o localiza el `ToolSet` por `namespace` crudo y falla si ya hay un tool con el mismo nombre **crudo** en ese namespace. |
| `list_functions()` | 448 | Catálogo de funciones (namespaces + firmas). |
| `get_function_details(GetFunctionDetailsInput)` | 474 | Firmas TS de las funciones pedidas. |
| `execute_typescript(code, ToolDisclosure, Option<PctxRegistry>)` | 604 | Delega en la versión con opciones (parche C2). |
| `execute_typescript_with_options(.., ExecuteTypescriptOptions)` | 623 | **CEREBRO-PATCH C2.** |
| `default_registry()` | 435 | Registry con las acciones del propio `CodeMode`. |

`ToolDisclosure` (`pctx_config/src/lib.rs:152-159`): `Catalog` (por defecto), `Filesystem` (alias `fs`) y `Sidecar`. En `Catalog` el código del modelo se concatena con los namespaces TS y `export default await run();`: el modelo define `async function run()` y la ejecución añade el `export default`. Si el modelo ya escribe `export default`, falla con TS2528.

### Nombres TS (WS3 → `ToolRecord::ts_name`)

- **Namespace visible** = `Case::Pascal.sanitize(namespace)` (`pctx_codegen/src/tools.rs:70-75`), que usa `heck::to_pascal_case` (`pctx_codegen/src/case.rs:24`). Sin namespace: `Tools` (`DEFAULT_NAMESPACE`). `github` → `Github`, `my-server` → `MyServer`. Test: `ts_naming_rules`, `ts_naming_without_namespace`.
- **Nombre de función** = `Case::Camel.sanitize(name)` (`tools.rs:139`, `case.rs:27`, `heck::to_lower_camel_case`): `create_issue` → `createIssue`.
- **Id del callback** = `"{namespace}__{name}"` con valores **crudos** (`pctx_code_mode/src/model.rs:225-234`): `github__create_issue`. Ese id es el que usan `PctxRegistry::add_callback` y `invokeInternal`.
- Conclusión para el hub: `CallbackConfig { namespace: Some(server.name), name: tool.name }` y calcular `ts_namespace`/`ts_name` con `pctx_code_mode::codegen::case::Case` (`Case::Pascal.sanitize`, `Case::Camel.sanitize`), **no** con `heck` suelto: `sanitize` además conserva guiones bajos iniciales/finales (`case.rs:34+`).
- **Colisiones.** pctx solo detecta duplicados por nombre crudo (`code_mode.rs:254-262`). Dos nombres crudos distintos con el mismo nombre TS (`create_issue` / `createIssue`) se registran sin error y **toda** ejecución falla después con TS2300/TS2393. Test: `ts_name_collisions_are_not_detected_at_registration`. Dos namespaces que coinciden en PascalCase (`my-server` / `my_server`) se acaban **fusionando** en un mismo `namespace` en el código generado (el test lo documenta: `[colisión de namespace]`, sin error). Por eso el hub deduplica (CONTRACTS §2).

## 2. `PctxRegistry` y `CallbackFn` (`crates/pctx_registry/src/registry.rs`)

- `CallbackFn` (l. 20) es un `Arc<dyn Fn(Option<Value>) -> Future<Result<Value, String>>>`. `PctxRegistry::add_callback(&self, id, CallbackFn)` (l. 186) toma `&self` (estado interno compartido).
- **Registry nuevo por ejecución:** sí, con el mismo `CodeMode`. Test `fresh_registry_per_execution` (dos ejecuciones con registries distintos devuelven resultados distintos).
- **Id ausente:** `execute_typescript*` comprueba que el registry contenga **todos** los ids de todos los `ToolSet` del `CodeMode` y, si falta alguno, devuelve `Err(Error::Message("Registry missing ids: [...]"))` (`code_mode.rs:667`) antes de ejecutar. Test `missing_registry_id_is_an_error`. Consecuencia: `build_registry` debe registrar **todo** el catálogo.
- **Error de callback:** `Err(String)` se envuelve en `RegistryError::ExecutionError("Failed calling callback with id \"{id}\": {e}")` (`registry.rs:259-263`). Dentro de TS llega como excepción con mensaje `Callback execution error: Failed calling callback with id "<id>": <s>` y es capturable con `try/catch`. Test `callback_error_is_catchable_in_typescript`.

## 3. Límites del executor (`crates/pctx_executor/src/lib.rs`)

| Tema | Resultado | Evidencia |
|---|---|---|
| Timeout por defecto | 120 s (`DEFAULT_TIMEOUT`, l. 41). `ExecuteOptions::with_timeout`. `CodeMode::execute_typescript` no lo exponía; ahora sí vía `ExecuteTypescriptOptions` (**C2**). | `code_mode.rs` (upstream) `ExecuteOptions::new().with_registry(registry)` |
| Cómo se corta | Hilo watchdog + `terminate_execution` (bucles) y `tokio::time::timeout` (promesas colgadas). Mensaje `Execution exceeded Ns and was stopped`. El proceso sigue sirviendo. | `infinite_loop_is_stopped_by_timeout` (2 s → cortó a 2,3 s) |
| Concurrencia | `V8_MUTEX` global (l. 33): una ejecución (type-check + ejecución) a la vez por proceso. | `lib.rs:33,192` |
| Heap de V8 | **Upstream: sin límite.** Una bomba de memoria aborta el proceso host (señal SIGTRAP, estado 5 en el test). **C1** añade el límite. | `memory_bomb_kills_the_host_process` (sin parche, hijo); `memory_bomb_is_stopped_with_heap_limit` (con parche) |
| Memoria fuera del heap | **No cubierta:** `new Uint8Array(3 GiB)` tiene éxito con `heap_limit=64 MB`. Riesgo residual (cgroup/`ulimit -v` o workers en la Fase 1). | `arraybuffer_allocation_behaviour` |
| Red | **Cero red en modo callbacks.** El runtime solo registra `op_invoke` y `op_sleep`; no hay `deno_fetch`/`deno_net`. `fetch`, `WebSocket`, `XMLHttpRequest`, `Deno.connect` son `undefined`; `fetch(...)` da `ReferenceError`/`TypeError`. | `sandbox_has_no_network_nor_fs`, `fetch_via_code_mode_fails` |
| Módulos y FS | `NoopModuleLoader` (`lib.rs:447`): todo `import()` (file/https) falla con "Module loading is not supported". Sin `Deno.readTextFile`/`open`. | `sandbox_has_no_network_nor_fs` |
| Salida | `console.*` capturado. El `stderr` de un fallo de ejecución es el mensaje de la excepción. Un `console.error` previo **sustituye** a ese mensaje en `stderr` (`pctx_executor/src/lib.rs` upstream l. 239-243, ahora 247-280); el error del callback sigue en `trace.events[]` (`callback_invocation`, `outcome.type == "error"`). | `console_error_hides_uncaught_error_message` |

### Type-check
- **Siempre** se ejecuta antes de ejecutar (`execute`, `lib.rs` ~l. 195), con el compilador TypeScript 5.3.3 en un runtime V8 aparte y sin caché entre ejecuciones. Los errores no relevantes se filtran (`is_relevant_error`).
- **Tiempo medido** (build de depuración, ejemplo con 1 callback): 277–318 ms por ejecución, **sin diferencia entre la primera y las siguientes** (la ejecución en sí cuesta ~10 ms). Test `type_check_always_runs_and_timing`. En `--release` es previsible que sea menor: **no medido**.
- Upstream no tenía timeout ni límite de heap en el type-check: **C3**.

### `ExecuteTypescriptOutput`
`{ success: bool, stdout: String, stderr: String, output: Option<Value>, registry, trace: ExecutionTrace }`. Los errores de tipos llegan como `success=false` con `stderr` = `Line L, Column C, TSnnnn: mensaje` por línea (`format_diagnostics`). Test `type_check_always_runs_and_timing` (`[type error]`).

### `CerebroErrorPayload::parse_from`
`Err(payload.to_callback_error())` en un callback llega a `stderr` como `… CEREBRO_ERROR:{json}` y `CerebroErrorPayload::parse_from(stderr)` lo extrae (incluido `approval_token`). Test `callback_error_reaches_stderr_and_parses`. Si un `console.error` previo lo oculta, se usa `trace.events[]` (CONTRACTS §2).

## 4. `pctx_config`
- `Config` (`pctx_config/src/lib.rs:12-47`) **no** lleva `deny_unknown_fields`: campos extra en la raíz se ignoran sin error.
- `StdioServerConfig` y `HttpServerConfig` (`server.rs:36,44`) **sí** llevan `#[serde(deny_unknown_fields)]`, y `ServerTransport` es `untagged`: un campo extra (por ejemplo `cwd`) en un servidor hace que **falle la deserialización** con un error poco claro de "data did not match any variant". Consecuencia: **no reutilizar `pctx_config::server` para nuestro `.cerebro/config.json`**; Cerebro define sus propios tipos (`DownstreamServer` en `cerebro_hub`, que ya tiene `cwd`). **No verificado con un test**: lectura de código.

## 5. `pctx_mcp_server`
Servidor MCP de pctx (rmcp **1.8**, features `server`, `transport-streamable-http-server`, `transport-io`, `macros`, `schemars`: `pctx_mcp_server/Cargo.toml:14-20`). `PctxMcpService` (`service.rs:44`, `#[tool_router]`) expone `list_functions` (l. 158), `get_function_details` (165), `execute_bash` (175) y `execute_typescript` (218), y reenvía `call_tool` a los upstream. Soporta stdio (`server.rs:142-204`) y HTTP con sesiones. Mantiene una `pool_cache` de conexiones por sesión.
**Utilidad para Cerebro:** como referencia de estructura (router de herramientas, instrumentación, sesiones), sí; como dependencia, **no**: usa rmcp 1.8 y la API de Cerebro (`search_functions`, `cerebro_*`, aprobaciones) es distinta. Cerebro tiene su propio servidor con rmcp 3.11 (`crates/cerebro`).

## 6. Rama upstream `search_functions`
- Un solo cambio real (3 commits de enero de 2026, último `40ca5f0`): `crates/pctx_code_mode/src/search.rs` con un índice **BM25 en memoria** (crate `bm25`) construido de forma perezosa sobre los `ToolSet` del `CodeMode`, más `CodeMode::search_functions(SearchFunctionsInput{query,k}) -> SearchFunctionsOutput` (= `ListFunctionsOutput` filtrado).
- **No tiene ancestro común con `upstream/main`** (historia reescrita; 462 commits propios) y no está integrada en la v1.0.0. No es mergeable tal cual; sería una reimplementación.
- **Solape con nuestro `search_functions`: ninguno bloqueante.** El nuestro busca en SQLite FTS5 sobre el catálogo persistente del hub (WS2), con filtros por servidor, anotaciones y detalle `names|summary|full`; el de upstream es un índice efímero por instancia de `CodeMode`, sin persistencia ni anotaciones. **Recomendación: no portarlo ni alinear firmas**; solo se puede tomar prestada la idea de devolver el código TS con el resultado. Revisar si upstream lo integra en una versión futura.

## 7. rmcp: versiones
- `upstream/main` (v1.0.0) declara `rmcp = "1.8.0"` en el workspace (`Cargo.toml:27`) y lo usan `pctx_mcp_server`, `pctx_registry` y otros.
- Cerebro usa **rmcp 3.11** declarada en cada crate `cerebro_*` (nunca `workspace = true`). Ambas versiones coexisten en el grafo (dos compilaciones de rmcp). Existe la rama upstream `dependabot/cargo/rmcp-2.1.0`, es decir, upstream va con retraso respecto a 3.11. **No verificado:** que los tipos de ambas versiones no se mezclen; no deben cruzar la frontera pctx ↔ hub (los callbacks trabajan con `serde_json::Value`).

## 8. Runbook de sincronización con upstream

```bash
git remote add upstream https://github.com/portofcontext/pctx.git   # una vez; NUNCA push a upstream
git fetch upstream --tags
git checkout -b sync/upstream-<tag> ccr-f6dcb0c5-1kktmp              # rama de integración, no la principal
git merge upstream/main                                              # o el tag: git merge v1.x.y
```
**Conflictos típicos**
- `README.md`: conservar **el nuestro**; el de upstream vive en `README.pctx.md` (actualizarlo si cambió).
- `Cargo.lock`: aceptar la versión de integración y regenerar con `cargo build -p cerebro_spike_pctx` (no editar a mano). El `cargo build` de todo el workspace descarga Swagger UI (`utoipa-swagger-ui`) y puede fallar sin red a GitHub: usar `-p`.
- `crates/pctx_*` donde haya `CEREBRO-PATCH`: resolver a mano conservando ambos cambios; localizar con `git grep -n CEREBRO-PATCH`.
- `.github/workflows/*` de upstream: se aceptan los cambios de upstream; los nuestros son `cerebro-*.yml` y no colisionan.

**Verificación tras el merge**
```bash
cargo test -p pctx_executor -p pctx_code_mode -p pctx_type_check_runtime   # tests upstream de los crates parcheados
cargo test -p cerebro_spike_pctx                                           # cubre los parches y las suposiciones de Cerebro
cargo test -p pctx_registry -p pctx_codegen -p pctx_config                 # resto de crates usados
```
Si falla un test del spike, **es un cambio de comportamiento de upstream que rompe una suposición de Cerebro**: actualizar este documento y avisar al coordinador antes de continuar. Si upstream integra algo equivalente a un parche (límite de heap, timeout…), retirar nuestro parche y quedarnos con el suyo.

**Política:** fusionar en cada versión menor de upstream (o cuando haya un parche de seguridad); nunca abrir PRs a upstream sin aprobación del usuario caso por caso.

## 9. Parches `CEREBRO-PATCH` aplicados

Todo cambio en `crates/pctx*` está marcado en el código con `// CEREBRO-PATCH: <motivo>`. Búscalos con `git grep CEREBRO-PATCH`. Los parches son **aditivos**: la API pública de pctx se conserva y los valores por defecto son los de upstream, salvo el timeout del type-check (C3).

| ID | Crate | Cambio | Motivo |
|---|---|---|---|
| C1 | `pctx_type_check_runtime`, `pctx_executor` | Límite de heap de V8: `heap_create_params`, `install_heap_guard` (near-heap-limit callback → `terminate_execution` y sube el límite un 50 % / 32 MB para evitar el abort fatal). `ExecuteOptions::heap_limit_bytes` + `with_heap_limit`. Por defecto `None` (V8 por defecto, como upstream). | Sin límite, una bomba de memoria aborta el proceso host (OOM fatal de V8). |
| C2 | `pctx_code_mode` | `CodeMode::execute_typescript_with_options(code, disclosure, registry, ExecuteTypescriptOptions { timeout, heap_limit_bytes })`. `execute_typescript` delega con `Default` (120 s, sin límite de heap). | `CodeMode` no exponía el timeout (`DEFAULT_TIMEOUT` fijo en `code_mode.rs`). |
| C3 | `pctx_type_check_runtime`, `pctx_executor` | `type_check_with_options(code, TypeCheckOptions { timeout, heap_limit_bytes })` con watchdog; nuevas variantes `TypeCheckError::Timeout` y `HeapLimitExceeded`. `ExecuteOptions::type_check_timeout` (por defecto `Some(60 s)`, `DEFAULT_TYPE_CHECK_TIMEOUT`). En `execute`, esos dos errores se devuelven como `ExecuteResult` fallido (stderr: `Type check exceeded …` / `Memory limit of N bytes exceeded …`). | El type-check no tenía timeout y retiene `V8_MUTEX`: un código patológico bloqueaba todo el proceso. |

Cambios de comportamiento respecto a upstream: **(1)** el type-check se corta a los 60 s por defecto; **(2)** se añaden campos públicos a `ExecuteOptions` (rompería solo a quien la construya con un literal de struct; en este repo nadie lo hace) y dos variantes a `TypeCheckError` (rompería un `match` exhaustivo externo; no hay ninguno en el repo).

Cobertura: `crates/cerebro_spike_pctx/tests/spike.rs` (`memory_bomb_is_stopped_with_heap_limit`, `code_mode_options_stop_memory_bomb_and_timeout`, `type_check_timeout_stops_the_check`, `type_check_heap_limit_survives`) y los tests upstream de `pctx_executor`, `pctx_code_mode` y `pctx_type_check_runtime`, que siguen en verde.

**Límite conocido (no cubierto):** el límite de heap de V8 no cubre memoria fuera del heap. Verificado: `new Uint8Array(3 GiB)` **tiene éxito** con `heap_limit = 64 MB`. Queda como riesgo residual; mitigación posible en la Fase 1: límite de memoria del proceso (cgroup/`ulimit -v`) o un pool de procesos worker.
