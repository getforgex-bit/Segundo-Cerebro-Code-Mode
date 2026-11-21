# WS3: Hub downstream, política y fixtures

**Rama:** `ws/ws3-hub` · **Coordinador:** `session_018vGgtDkbBcKozrRZMWdwQz` · **Lee antes:** `../PROTOCOL.md`, `../CONTRACTS.md` (§2 policy y hub, §5 y §6), `../PLAN.md` §V2.5 (idea clave), §V2.7-0d/0e y Anexo A §6 y §9, y `crates/cerebro_types/src/lib.rs`.

## Objetivo

Que **toda** llamada a un servidor MCP downstream pase por nuestro hub: política, aprobación, auditoría y ciclo de vida. Cada herramienta upstream se registra en pctx como **callback**, nunca con `add_mcp`.

## Alcance

**Incluye:**

H1. **`packages/fixtures`** (paquete TS, `@modelcontextprotocol/sdk`, `tsc` → `dist/`), con los servidores `echo` y `destructive` de `CONTRACTS.md` §5. Script `build` y un smoke test (`node dist/echo.js` responde a `tools/list`). El workspace pnpm ya existe en `packages/`. Envía `[LISTO PARA INTEGRAR WS3] fixtures` pronto, porque WS4 los necesita.

H2. **`crates/cerebro_policy`**: `Rule`, `PolicyConfig` y `Policy::decide`, con la precedencia de `CONTRACTS.md` §2. Tests de cada regla y de que gane el override más específico.

H3. **`crates/cerebro_hub`**, con rmcp **3.11** (`client`, `transport-child-process`):
- `Hub::new` y `sync_catalog`:
  - lanza cada servidor stdio, hace `tools/list` y construye `ToolRecord`: `schema_hash` en blake3, `approx_tokens` ≈ bytes del JSON / 4, y `ts_name` según el mapeo que documente WS1;
  - persiste con `Catalog::replace_server_tools`;
  - un servidor caído no rompe el resto: queda `status=Error` con su `last_error`.
- `call`:
  1. busca la herramienta (si no existe → `TOOL_NOT_FOUND` con sugerencias por prefijo o distancia);
  2. aplica `Policy::decide`; con `Confirm`, mira si `ctx.approvals` trae un token válido (`ApprovalStore::consume`) y, si no, crea una aprobación → `NEEDS_APPROVAL` con `approval_token`; con `Deny` → `DENIED`;
  3. arranque perezoso del servidor si está parado;
  4. `tools/call` con `call_timeout`;
  5. el resultado se mapea a `Value`: `structuredContent` si existe; si no, `{ text, content }`. `isError` → `TOOL_ERROR`;
  6. `AuditLog::record_tool_call`, con argumentos redactados y hasheados;
  7. se acumula `CallSummary` en el `ExecutionScope`.
- `callback_configs` y `build_registry`: un `PctxRegistry` **nuevo por ejecución**, donde cada id de callback ejecuta `hub.call(...)` con el `CallCtx` de esa ejecución.
- `shutdown` cierra los procesos hijos (sin zombis).

H4. **Tests de integración** (con `Store::open_in_memory()` cuando WS2 lo integre; mientras, mocks propios de los traits):
- sincroniza `echo` y `destructive`;
- `call echo.echo` → ok y queda auditado;
- `call destructive.delete_item` → `NEEDS_APPROVAL`; con el token → ok; con el mismo token otra vez → `NEEDS_APPROVAL`;
- servidor inexistente → `status=Error` y el resto funciona;
- **TS en el sandbox de pctx** llama a `echo.echo()` a través de `build_registry`. Este es el test clave de la arquitectura.

H5. **`docs/cerebro/adr/0005-hub-callbacks.md`** y **`docs/cerebro/threat-model.md`**: activos, actores, superficies (sandbox, puente, downstream, aprobaciones, skills, índices y modo remoto futuro), amenazas STRIDE-lite y controles existentes o pendientes, incluida la política de flujo M5.

**No incluye:** el transporte HTTP downstream, OAuth, taint completo (Fase 2) ni MRTR. Investiga si rmcp 3.11 permite devolver `InputRequiredResult` desde el **servidor** (lo hará WS4) y anótalo en el ADR 0005.

## Dependencias

- Del mapeo de nombres TS y del formato de error de callback de **WS1**. Mientras llega, usa `CallbackConfig::id()` y asume que el error llega a `stderr` tal cual.
- Del **store de WS2**: usa mocks hasta que el coordinador lo integre.

## Consultas previstas

- Cualquier cambio en `cerebro_types` (por ejemplo, si `CallCtx` necesita más campos).
- La estrategia de concurrencia de los clientes rmcp (una conexión por servidor compartida o un pool).
- Si pctx exige el registry completo antes de `execute_typescript` (`Registry missing ids`) y eso choca con el registry por ejecución.

## Primer paso

`packages/fixtures` (H1): commit y aviso. Después la política (H2), que no tiene dependencias, y luego el hub.
