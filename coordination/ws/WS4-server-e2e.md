# WS4: Servidor MCP `cerebro` y E2E

**Rama:** `ws/ws4-server` · **Coordinador:** `session_018vGgtDkbBcKozrRZMWdwQz` · **Lee antes:** `../PROTOCOL.md`, `../CONTRACTS.md` (§2 cerebro, §3, §4 y §6), `../PLAN.md` §V2.5, §V2.7-0f/0g y Anexo A §4–§6, y `crates/cerebro_types/src/lib.rs`.

## Objetivo

El binario que conectan los clientes IA. Un servidor MCP con rmcp **3.11** por stdio, que expone **exactamente 6 herramientas** y compone el store, la búsqueda, el hub y el motor pctx. Además, el E2E que demuestra la arquitectura de punta a punta.

## Alcance

**Incluye:**

H1. **`crates/cerebro`** (bin), con `clap`: `cerebro serve --stdio [--project] [--config]` y `cerebro sync`.
- Parser de `.cerebro/config.json` (`CONTRACTS.md` §2).
- Logs a **stderr**, nunca a stdout, porque stdout es el canal MCP.
- `instructions` MCP breves: "Use search_functions to find tools, get_function_details for signatures, then compose calls in execute_typescript".

H2. **Las 6 herramientas** de `CONTRACTS.md` §3, con descripciones en inglés y concisas: **el total de `tools/list` debe quedar ≤ 2.000 tokens, aproximados como bytes / 4**.
- `search_functions` → `ToolSearch::search` + `format_hits`.
- `get_function_details` → `CodeMode::get_function_details`.
- `execute_typescript`:
  1. crea un ULID;
  2. `hub.build_registry(id, approvals)`;
  3. `CodeMode::execute_typescript(code, ToolDisclosure::Catalog, Some(registry))`;
  4. recorta `output` y `stdout` a `max_result_bytes`, con `truncated: true`;
  5. extrae `error` con `CerebroErrorPayload::parse_from(stderr)`;
  6. añade `calls` del `ExecutionScope`;
  7. `AuditLog::record_execution`.
- `cerebro_recall` y `cerebro_remember` son stubs. `cerebro_primer` es **real** en lo básico: servidores, su estado y el número de herramientas.
- Al arrancar: `hub.sync_catalog()`, y después `CodeMode::add_callback` para cada `hub.callback_configs()`.

H3. **`packages/e2e`** (TS, `@modelcontextprotocol/sdk` como cliente y `vitest`), contra el binario compilado (`target/debug/cerebro`) con un proyecto temporal y los fixtures de WS3:
- `tools/list` devuelve 6 herramientas; se calcula y **registra** su tamaño en bytes (y se comprueba que es ≤ 8.000 bytes);
- `search_functions("echo text")` devuelve `echo.echo` primero;
- `execute_typescript` con código que llama a `echo` dos veces y devuelve solo un resumen → `success`, y `calls` tiene 2;
- `destructive.delete_item` → `error.code == "NEEDS_APPROVAL"`; repetir con `approvals: [token]` → ok;
- código con error de tipos → diagnóstico legible, sin colgarse.

H4. **Docs** en `docs/cerebro/`:
- `meta-tools.md`: schemas exactos y descripciones de las 6 herramientas, con su tamaño medido.
- `clients.md`: cómo registrar `cerebro serve --stdio` en Claude Code (`claude mcp add` / `.mcp.json`), Claude Desktop (`claude_desktop_config.json`), Codex (`~/.codex/config.toml`, `[mcp_servers.cerebro]`) y Antigravity 2.0 (`~/.gemini/config/mcp_config.json` o `.agents/mcp_config.json`). Incluye el aviso de que el cliente debe **dejar de cargar los servidores originales** para que haya ahorro, y el aviso de seguridad sobre las allowlists.
- `sandbox-api.d.ts`: la API que ve la IA según las convenciones reales de pctx (namespaces), validada con `tsc --noEmit --strict`.

**No incluye:** Streamable HTTP, `install` o `import-config` (Fase 1), ni el cerebro real (Fase 2). Investiga si el **servidor** rmcp 3.11 puede devolver `InputRequiredResult` (MRTR) para las aprobaciones; si es viable y barato, propónlo por consulta antes de implementarlo.

## Dependencias y mocks

- Hasta que WS2 y WS3 se integren, usa implementaciones *mock* de `Catalog`, `ToolSearch`, `AuditLog` y `ApprovalStore` en `crates/cerebro/src/mock.rs`, solo para tests. Mantén el cableado detrás de esos traits para que el cambio a la implementación real sea de pocas líneas.
- Fixtures de WS3: el coordinador te avisará con `[INTEGRADO]`.

## Consultas previstas

- Cómo envolver el `Hub` si su API no encaja con el servidor rmcp (lifetimes o `Arc`).
- Si conviene exponer `list_functions` de pctx.
- MRTR.

## Primer paso

H1 y las descripciones de las 6 herramientas con mocks, más un test Rust que haga `tools/list` con el cliente rmcp sobre un duplex en memoria y mida el tamaño. Envía un `[REPORTE WS4]` con ese tamaño.
