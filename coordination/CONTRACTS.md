# Contratos entre líneas de trabajo

> Lo que aquí se describe en prosa vive como código en `crates/cerebro_types`, que es la fuente de verdad. Cambios: `[CONSULTA]` al coordinador. Una firma pública de otra WS no cambia sin avisar.

## 1. Crates y dependencias

```
cerebro_types   (coordinador)  tipos + traits; deps: serde, serde_json, anyhow
cerebro_store   (WS2)          impl Catalog, AuditLog, ApprovalStore           → cerebro_types
cerebro_search  (WS2)          impl ToolSearch + formato de resultados         → cerebro_types, cerebro_store
cerebro_policy  (WS3)          Policy::decide                                  → cerebro_types
cerebro_hub     (WS3)          downstream rmcp 3.11 + callbacks pctx           → cerebro_types, cerebro_policy, pctx_code_mode
cerebro         (WS4, bin)     servidor MCP rmcp 3.11 + CLI                    → todos + pctx_code_mode
```

Los crates nuevos viven en `crates/cerebro_<nombre>/`. El workspace los incluye solo (`members = ["crates/*"]`). Licencia en su `Cargo.toml`: `license = "LicenseRef-ForgeX-Internal"` y `publish = false`.

**Versiones fijadas:**

| Crate | Versión | Notas |
|---|---|---|
| `rmcp` | `3.11` | `default-features = false`. Hub: `["client", "transport-child-process"]`. Servidor: `["server", "macros", "transport-io"]` (+ `transport-streamable-http-server` en la fase 1). **Nunca** `workspace = true`, porque eso sería la 1.8 de pctx. |
| `rusqlite` | `0.41` | `features = ["bundled"]`; es la que exige `rusqlite_migration 2.7`. |
| `rusqlite_migration` | `2.7` | |
| `blake3` | `1.8` | Todos los hashes: hex en minúsculas. |
| `ulid` | `3` | Identificadores de ejecución y memoria. |
| `pctx_code_mode` | `{ path = "../pctx_code_mode" }` | Motor Code Mode. |
| `serde`, `serde_json`, `anyhow`, `thiserror`, `tokio`, `tracing` | `workspace = true` | |

## 2. API pública por crate (firmas mínimas obligatorias)

### `cerebro_store` (WS2)

```rust
pub struct StorePaths { pub brain_db: PathBuf, pub hub_db: PathBuf }
#[derive(Clone)] pub struct Store { /* Arc<Mutex<rusqlite::Connection>> */ }
impl Store {
    pub fn open(paths: &StorePaths) -> Result<Self>;   // crea los dirs, aplica pragmas y migraciones, ATTACH hub
    pub fn open_in_memory() -> Result<Self>;           // para tests (ambas BD en memoria)
}
impl Catalog for Store {}  impl AuditLog for Store {}  impl ApprovalStore for Store {}
```

### `cerebro_search` (WS2)

```rust
pub struct SqliteToolSearch { /* Store */ }
impl SqliteToolSearch { pub fn new(store: Store) -> Self; }
impl ToolSearch for SqliteToolSearch {}
/// Texto compacto para `search_functions`: una línea por resultado. Ver §4.
pub fn format_hits(hits: &[ToolHit], detail: Detail) -> String;
```

### `cerebro_policy` (WS3)

```rust
#[derive(Deserialize)] #[serde(rename_all = "snake_case")] pub enum Rule { Allow, Confirm, Deny }
#[derive(Deserialize, Default)] pub struct PolicyConfig {
    pub default_unknown: Option<Rule>,            // por defecto Confirm
    pub overrides: BTreeMap<String, Rule>,        // "server.tool" o "server.*"; gana el más específico
}
pub struct Policy { /* … */ }
impl Policy {
    pub fn new(cfg: PolicyConfig) -> Self;
    pub fn decide(&self, tool: &ToolRecord, ctx: &CallCtx) -> Decision;
}
```

Reglas por defecto: override > `readOnlyHint == true` → Allow > `destructiveHint == true` → Confirm > `default_unknown`. La regla de taint (M5) llega en la Fase 2.

### `cerebro_hub` (WS3)

```rust
#[derive(Deserialize, Clone)] pub struct DownstreamServer {
    pub name: String, pub command: String,
    #[serde(default)] pub args: Vec<String>,
    #[serde(default)] pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}
pub struct HubConfig { pub servers: Vec<DownstreamServer>, pub call_timeout: Duration, pub idle_shutdown: Duration }
pub struct Hub { /* … */ }
pub struct SyncReport { pub servers_ok: Vec<String>, pub servers_failed: Vec<(String, String)>, pub tools: usize }
pub struct ExecutionScope { pub execution_id: String, /* calls: Arc<Mutex<Vec<CallSummary>>> */ }
impl ExecutionScope { pub fn calls(&self) -> Vec<CallSummary>; pub fn raw_bytes(&self) -> u64; }

impl Hub {
    pub fn new(cfg: HubConfig, catalog: Arc<dyn Catalog>, audit: Arc<dyn AuditLog>,
               approvals: Arc<dyn ApprovalStore>, policy: Policy) -> Arc<Self>;
    /// Arranca cada servidor (stdio), `tools/list` → catálogo. No falla si cae un servidor.
    pub async fn sync_catalog(self: &Arc<Self>) -> Result<SyncReport>;
    /// Configs para `CodeMode::add_callback`, una por herramienta del catálogo.
    pub fn callback_configs(&self) -> Result<Vec<pctx_code_mode::model::CallbackConfig>>;
    /// Registry nuevo **por ejecución**: cada callback → política → aprobación → llamada → auditoría.
    pub fn build_registry(self: &Arc<Self>, execution_id: &str, approvals: Vec<String>)
        -> Result<(pctx_code_mode::PctxRegistry, ExecutionScope)>;
    /// Llamada directa (la usan los callbacks y los tests).
    pub async fn call(self: &Arc<Self>, server: &str, tool: &str, args: Option<Value>, ctx: &CallCtx)
        -> std::result::Result<Value, CerebroErrorPayload>;
    pub async fn shutdown(&self);
}
```

Mapeo a pctx: `CallbackConfig { namespace: Some(server.ts_namespace), name: tool.name, description, input_schema, output_schema }`. El id del callback es el de pctx (`CallbackConfig::id()` = `"{namespace}__{name}"`). El nombre TS real que genera pctx se guarda en `ToolRecord::ts_name`. WS1 confirma cómo lo calcula pctx.

**Errores de callback:** `Err(payload.to_callback_error())` con `CerebroErrorPayload` (prefijo `CEREBRO_ERROR:`). Para `NEEDS_APPROVAL`, el payload lleva `approval_token`.

### `cerebro` (WS4, binario)

```
cerebro serve --stdio [--project <dir>] [--config <ruta>]   # por defecto <dir>/.cerebro/config.json
cerebro sync [--project <dir>]                                # sincroniza el catálogo e imprime el SyncReport
```

Config de la Fase 0 (`.cerebro/config.json`): la parsea WS4 componiendo los tipos de WS3.

```json
{
  "servers": [{ "name": "echo", "command": "node", "args": ["packages/fixtures/dist/echo.js"] }],
  "policy": { "default_unknown": "confirm", "overrides": { "echo.*": "allow" } },
  "limits": { "timeout_ms": 10000, "max_result_bytes": 20000 }
}
```

Rutas: `brain_db = <project>/.cerebro/brain.db`, `hub_db = <project>/.cerebro/hub.db`. En la Fase 0 todo es local al proyecto; mover `hub.db` a un directorio global queda para la Fase 1.

## 3. Superficie MCP de la Fase 0 (nombres fijos)

| Herramienta | Entrada | Salida (texto) |
|---|---|---|
| `search_functions` | `{ query: string, detail?: "names"\|"summary"\|"full", server?: string, limit?: number }` | `format_hits(...)` |
| `get_function_details` | `{ functions: string[] }` (`"namespace.fn"`) | Firmas TS que devuelve `CodeMode::get_function_details` |
| `execute_typescript` | `{ code: string, approvals?: string[] }` | JSON `{ success, output, stdout, error?, calls: CallSummary[], truncated }` |
| `cerebro_recall` | `{ query: string, scope?: string, limit?: number }` | Stub en la Fase 0: `"brain not implemented yet (phase 2)"` |
| `cerebro_remember` | `{ kind: string, title: string, body: string }` | Stub |
| `cerebro_primer` | `{ focus?: string }` | Stub con la lista de servidores y el número de herramientas (real) |

En `execute_typescript`, `error` es un `CerebroErrorPayload` si se pudo extraer de `stderr` (`CerebroErrorPayload::parse_from`); si no, el texto de `stderr` recortado.

## 4. Formato de `search_functions` (lo implementa WS2 en `format_hits`)

```
names:    echo.echo
summary:  echo.echo — Echoes the input text [read]
full:     echo.echo — Echoes the input text [read]
            echo(input: { text: string }): Promise<{ text: string }>
```

Marcas: `[read]` si `readOnlyHint`, `[destructive]` si `destructiveHint`, y `[write]` en el resto. La descripción se recorta a la primera frase o a 100 caracteres.

## 5. Fixtures MCP (WS3; WS4 los consume)

- Paquete `packages/fixtures`, en TS con `@modelcontextprotocol/sdk`, compilado en `packages/fixtures/dist/<nombre>.js`, ejecutable con `node`.
- `echo`:
  - `echo({text})` → texto y `structuredContent {text}`, con `readOnlyHint: true`;
  - `add({a,b})` → `{sum}`, con `readOnlyHint: true`.
- `destructive`:
  - `read_item({id})`, con `readOnlyHint: true`;
  - `delete_item({id})`, con `destructiveHint: true`.
- Más adelante: `large-output` (resultado de 1 MB) e `injection` (devuelve un texto que intenta instruir a la IA).

## 6. Convenciones

- **Errores:** `anyhow` en los traits (`cerebro_types::Result`) y `thiserror` dentro de cada crate si aporta.
- **Hashes:** blake3 hex. Los argumentos se hashean como JSON canónico, con claves ordenadas y sin espacios.
- **Tiempo:** RFC 3339 UTC en SQLite (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`).
- **Identificadores:** ULID en minúsculas.
- **Tests:**
  - sin red;
  - los fixtures se lanzan con `node` desde `packages/fixtures/dist`;
  - si `dist` no existe, el test se marca `#[ignore]` con el motivo, **nunca** como un falso verde.
