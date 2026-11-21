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

Reglas por defecto: override (`server.tool` > `server.*` > `*`) > **si `readOnlyHint` y `destructiveHint` son ambos `true` → Confirm** (las anotaciones son pistas no confiables y en caso de conflicto gana lo más restrictivo) > `readOnlyHint == true` → Allow > `destructiveHint == true` → Confirm > `default_unknown`. La regla de taint (M5) llega en la Fase 2.

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
        -> Result<(pctx_code_mode::registry::PctxRegistry, ExecutionScope)>;
    /// Llamada directa (la usan los callbacks y los tests).
    pub async fn call(self: &Arc<Self>, server: &str, tool: &str, args: Option<Value>, ctx: &CallCtx)
        -> std::result::Result<Value, CerebroErrorPayload>;
    pub async fn shutdown(&self);
}
```

Mapeo a pctx: `CallbackConfig { namespace: Some(server.name), name: tool.name, description, input_schema, output_schema }`. El id del callback es el de pctx (`CallbackConfig::id()` = `"{namespace}__{name}"`).
**pctx convierte el namespace a PascalCase** (`echo` → `Echo`, ver `pctx_codegen/src/tools.rs:70`; lo halló WS4). Por tanto, `ServerRecord::ts_namespace` y el nombre TS de cada herramienta (`ToolRecord::ts_name`) **se calculan con las mismas funciones de `pctx_codegen`**, reutilizadas y no reimplementadas, para que coincidan siempre con lo que ve la IA (`await Echo.echo({...})`). Si esas funciones no son públicas, WS1 propone el parche mínimo.

**Convención de nombres (verificada por WS1 en `cerebro_spike_pctx`):**
- `ServerRecord::ts_namespace` = **PascalCase** visible (`heck::to_pascal_case(server.name)`: `github` → `Github`, `my-server` → `MyServer`).
- `ToolRecord::ts_name` = `to_lower_camel_case(tool.name)` (`create_issue` → `createIssue`).
- `CallbackConfig.namespace` = `server.name` **crudo**, y `name` = nombre MCP crudo, así que el id es `"{server}__{tool}"`.
- Se usan las funciones de `pctx_code_mode::codegen::case`, no una reimplementación.

**Colisiones:** pctx no detecta que dos nombres crudos produzcan el mismo nombre TS (`create_issue` y `createIssue` hacen fallar **toda** ejecución con TS2300).
- El hub deduplica por (`ts_namespace`, `ts_name`) antes de registrar: ordena por nombre crudo, se queda con el primero y marca los demás `available=false`, con un aviso en `SyncReport`.
- Dos servidores cuyo PascalCase coincida (`my-server` y `my_server`): el segundo se rechaza en la validación de la config, porque nombres de servidor distintos deben dar namespaces distintos.

**Registry completo:** `execute_typescript` exige que el registry contenga **todos** los ids del `CodeMode`, así que `build_registry` registra el catálogo entero. Si el catálogo cambia, el servidor reconstruye el `CodeMode`.

**Extracción de errores (WS4):** `CerebroErrorPayload::parse_from(stderr)`. Si no aparece, porque un `console.error` previo lo tapa (`pctx_executor/src/lib.rs:239`), se busca en `output.trace.events[]` el `callback_invocation` con `outcome.type == "error"`.

**Concurrencia:** pctx serializa todas las ejecuciones con un `V8_MUTEX` global, así que solo corre una a la vez por proceso. Se acepta en la Fase 0; en la Fase 1 se evalúa un pool de procesos worker.

**Runtime de los callbacks (aviso de WS4):** `Hub::new` debe llamarse dentro del runtime tokio principal y guardar `tokio::runtime::Handle::current()`. `execute_typescript` corre en `spawn_blocking`, con un runtime `current_thread` propio, porque Deno no es `Send`. Los futures de los callbacks se ejecutan **en ese runtime efímero**. Por eso el callback del hub debe hacer `main_handle.spawn(hub.call(...)).await`, donde `main_handle` es el `tokio::runtime::Handle` del runtime principal capturado al construir el registry. Así nada que tenga que sobrevivir a la ejecución (procesos hijos o tareas del cliente rmcp) nace en el runtime efímero.

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
  "limits": { "timeout_ms": 10000, "max_result_bytes": 20000 },
  "approvals": { "mode": "out_of_band" }
}
```

### Aprobaciones (decisión de seguridad del coordinador, a raíz del threat-model de WS2)

La IA **ve** el token en `NEEDS_APPROVAL`. Si bastara con reenviarlo en `approvals`, una IA manipulada por inyección de prompts se autoaprobaría. Por eso:

- `ApprovalMode::OutOfBand` **(por defecto)**: `consume` solo acepta tokens en estado `approved`. Un humano aprueba fuera de banda con **`cerebro approve <token>`**.
  - Este comando muestra la herramienta, el motivo y la expiración, y pide `y/N`.
  - **Se niega a ejecutarse si stdin/stdout no son una TTY interactiva**, porque las herramientas bash de los agentes no tienen TTY. No es infalible, pero sube el listón junto con los permisos de bash del cliente.
  - `cerebro approvals` lista las pendientes.
- `ApprovalMode::ClientConfirm` (opt-in, **más débil**): también acepta `pending`. Confía en que el cliente pida confirmación humana para cada `execute_typescript`.
- El mensaje de `NEEDS_APPROVAL` (hub) debe decir literalmente: `"Approval required: ask the user to run \`cerebro approve <token>\` in an interactive terminal, then retry with approvals: [\"<token>\"]"`.
- **Fase 1, prioridad 1:** MRTR (`CallToolResponse::InputRequired`), para que la confirmación la muestre la UI del cliente al humano y no pase por la IA.
- Firma: `ApprovalStore::consume(token, tool, args_hash, mode)` y `ApprovalStore::list_pending()`. Ya está en `cerebro_types` y `cerebro_store`.

Rutas: `brain_db = <project>/.cerebro/brain.db`, `hub_db = <project>/.cerebro/hub.db`. En la Fase 0 todo es local al proyecto; mover `hub.db` a un directorio global queda para la Fase 1.

## 3. Superficie MCP de la Fase 0 (nombres fijos)

| Herramienta | Entrada | Salida (texto) |
|---|---|---|
| `search_functions` | `{ query: string, detail?: "names"\|"summary"\|"full", server?: string, limit?: number }` | `format_hits(...)` |
| `get_function_details` | `{ functions: string[] }`: acepta `Namespace.fn` (lo que muestra `search_functions`) **y también** `server.tool`, que se traduce vía catálogo | Firmas TS que devuelve `CodeMode::get_function_details` |
| `execute_typescript` | `{ code: string, approvals?: string[] }` | JSON `{ success, output, stdout, error?, calls: CallSummary[], truncated }` |
| `cerebro_recall` | `{ query: string, scope?: string, limit?: number }` | Stub en la Fase 0: `"brain not implemented yet (phase 2)"` |
| `cerebro_remember` | `{ kind: string, title: string, body: string }` | Stub |
| `cerebro_primer` | `{ focus?: string }` | Stub con la lista de servidores y el número de herramientas (real) |

En `execute_typescript`, `error` es un `CerebroErrorPayload` si se pudo extraer de `stderr` (`CerebroErrorPayload::parse_from`); si no, el texto de `stderr` recortado.

## 4. Formato de `search_functions` (lo implementa WS2 en `format_hits`)

El namespace llega en `ToolHit::ts_namespace`, que la búsqueda rellena con un JOIN a `servers`; `format_hits` no lo recalcula. **El identificador que se muestra es `ts_namespace.ts_name`** (namespace en PascalCase, como lo genera pctx): el mismo que la IA usa en el código (`await Echo.echo(...)`) y en `get_function_details`. No se muestra el nombre MCP original. Decisión del coordinador del 2026-11-21, a raíz de una consulta de WS2.

```
names:    Github.createIssue
summary:  Github.createIssue — Create an issue in a repository [write]
full:     Github.createIssue — Create an issue in a repository [write]
            createIssue(input: { owner: string; repo: string; title: string }): Promise<unknown>
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

## 7. Proceso de cambio de contrato (entre equipos)

Aplica a `crates/cerebro_types/` y a este documento:
1. Issue con la plantilla `cerebro-contract-change`: qué cambia, por qué, quién lo consume, compatibilidad y plan de migración.
2. PR con el cambio, que **compile en `develop` sin romper a los consumidores**. Si rompe algo, el mismo PR adapta a los consumidores o se coordina el orden en el Issue.
3. **Visto bueno explícito de ambos coordinadores** (Opus y Neon) en el Issue.
4. Merge por Opus y aviso a los equipos afectados (comentario en el Issue y `[INFO]` interno en el Equipo A).

Los cambios **aditivos** (un campo opcional, un método con implementación por defecto) siguen el mismo flujo, pero basta un solo visto bueno si el otro coordinador no responde en 24 h.

## 8. Contratos pendientes (los propone el Equipo B)

| Contrato | Issue | Estado | Consumidor |
|---|---|---|---|
| Trait `Brain` (remember, recall con scope `memories\|docs\|code\|all`, primer con presupuesto de tokens, get, link, neighbors, read, symbols) | B1 | Por proponer (Neon) | Equipo A: handlers `cerebro_recall/remember/primer` y namespace `Brain` en el sandbox |
| API de `cerebro_clients` (detectar, leer servidores → `DownstreamServer`, instalar o reemplazar con `--dry-run` y diff, backup) | B4 | Por proponer (Neon / `@clients`) | Equipo A: CLI `cerebro import-config / install / doctor` |
