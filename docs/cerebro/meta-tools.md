# Meta-herramientas de `cerebro` (Fase 0)

El servidor MCP `cerebro` expone **exactamente 6 herramientas**, sin importar cuántos servidores downstream haya detrás. Las herramientas downstream no aparecen en `tools/list`: la IA las descubre con `search_functions` y las llama dentro de `execute_typescript`.

**Tamaño medido de `tools/list`** (`serde_json::to_vec(&tools)`, test `server::tests::lists_six_tools_and_measures_size` y E2E): **2096 bytes ≈ 524 tokens** (bytes/4). Objetivos: ≤ 2.000 tokens (aprox.) y ≤ 8.000 bytes. Los esquemas se escriben a mano en `crates/cerebro/src/tools.rs` (no con `schemars`) precisamente para controlar este tamaño.

`instructions` MCP: *"Use search_functions to find tools, get_function_details for signatures, then compose calls in execute_typescript."*

| Herramienta | Entrada (JSON Schema) | Salida |
|---|---|---|
| `search_functions` | `{query: string, detail?: "names"\|"summary"\|"full", server?: string, limit?: 1..25}` (requerido: `query`) | Una línea por resultado: `Namespace.fn — resumen [read\|write\|destructive]` (`format_hits`, CONTRACTS §4). Si no hay resultados: `No matching functions.` |
| `get_function_details` | `{functions: string[]}` | Firmas TypeScript (`CodeMode::get_function_details`). Acepta `Namespace.fn` y también `server.tool` (se traduce vía catálogo). |
| `execute_typescript` | `{code: string, approvals?: string[]}` | JSON `{success, output, stdout, error?, calls, truncated}` |
| `cerebro_recall` | `{query: string, scope?: string, limit?: integer}` | Stub: `brain not implemented yet (phase 2)` |
| `cerebro_remember` | `{kind: string, title: string, body: string}` | Stub (igual) |
| `cerebro_primer` | `{focus?: string}` | Servidores, estado y número de herramientas (real) |

Anotaciones MCP: `search_functions`, `get_function_details`, `cerebro_recall` y `cerebro_primer` llevan `readOnlyHint: true`.

## Descripciones (literales)

- `search_functions`: "Search available tool functions. One line per match: `namespace.fn — summary [read|write|destructive]`."
- `get_function_details`: "Get TypeScript signatures for functions (e.g. ["echo.echo"]). Call before using them in execute_typescript."
- `execute_typescript`: "Run TypeScript in a sandbox. Define `async function run() { ... return x; }`; call tools as `await Namespace.fn(args)`. Only the return value reaches you, so filter and aggregate in code. Returns JSON {success, output, stdout, error?, calls, truncated}. If error.code is NEEDS_APPROVAL, follow error.message (the user must approve), then rerun with approvals: [error.approval_token]."
- `cerebro_recall`: "Search project memory: decisions, conventions, docs and code."
- `cerebro_remember`: "Save a project memory (decision, convention, fact, task)."
- `cerebro_primer`: "Project overview: connected tool servers, their status and tool counts. Call at session start."

## `execute_typescript`

1. Crea un ULID de ejecución y un `PctxRegistry` nuevo por ejecución (`hub.build_registry`).
2. Ejecuta el código con `CodeMode::execute_typescript(.., ToolDisclosure::Catalog, ..)` en un hilo bloqueante con su propio runtime `current_thread` (Deno no es `Send`).
3. Recorta `output` y `stdout` a `limits.max_result_bytes` respetando los límites de carácter UTF-8; si recorta, `truncated: true`. Un `output` que excede el límite se devuelve como cadena JSON recortada.
4. `error`: un `CerebroErrorPayload` (`code`, `message`, `approval_token?`, `suggestions?`) si `stderr` lo contiene (`CEREBRO_ERROR:{...}`); si no, el texto de `stderr` recortado (p. ej. diagnósticos de tipos).
5. `calls`: `[{tool, ms, ok, decision, result_bytes}]` de cada llamada a una herramienta downstream.
6. Audita la ejecución (`code_hash = blake3(code)`, estado, bytes, duración).

**Límites:** `limits.timeout_ms` (por defecto 10 000) y `limits.heap_limit_mb` (256) se aplican con `CodeMode::execute_typescript_with_options` (parche CEREBRO-PATCH de WS1): al vencer el plazo el executor detiene el script y `error.code` es `TIMEOUT`; el heap de V8 tiene un tope duro. El E2E comprueba que `while(true){}` con `timeout_ms: 2000` devuelve el error en < 5 s y que el servidor sigue vivo.

## Aprobaciones

Ver `clients.md` (sección "Aprobaciones") y `CONTRACTS.md` §2. Resumen: una herramienta que la política marca como `confirm` devuelve `NEEDS_APPROVAL` con un `approval_token`. Con el modo por defecto (`out_of_band`), un humano debe ejecutar `cerebro approve <token>` en una terminal interactiva antes de que la IA reintente con `approvals: [token]`.
