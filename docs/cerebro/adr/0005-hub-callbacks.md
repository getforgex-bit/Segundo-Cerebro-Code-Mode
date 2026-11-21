# ADR-0005: Hub downstream con callbacks de pctx

- **Estado:** propuesta (2026-11-21) · **Dueño:** WS3

## Contexto

Cerebro debe interponerse en **toda** llamada a un servidor MCP downstream para aplicar política, aprobaciones, auditoría y ciclo de vida. pctx sabe hablar con servidores MCP por su cuenta (`add_mcp`, con `rmcp 1.8` interno), pero eso se salta cualquier control nuestro.

## Decisión

1. **Callbacks, no `add_mcp`.** Cada herramienta del catálogo se registra con `PctxRegistry::add_callback("{server}__{tool}", …)`. El sandbox TS solo puede llegar a un downstream a través de `Hub::call`. No se parchea el registry de pctx.
2. **Cliente propio con `rmcp 3.11`** (`client` + `transport-child-process`), solo stdio en la Fase 0.
3. **Una conexión por servidor**, con un `tokio::Mutex` que evita el doble spawn. Se comparte el `Peer` (rmcp multiplexa peticiones), de modo que varias llamadas concurrentes al mismo servidor no se serializan. Un pool no aporta nada con servidores stdio.
4. **Arranque perezoso y apagado por inactividad.** `sync_catalog` arranca los servidores para listar herramientas; una tarea *reaper* cierra los que pasan `idle_shutdown` sin uso y sin llamadas en vuelo. La siguiente llamada los relanza. `shutdown()` cierra todo (`ChildWithCleanup` mata el proceso al soltarse, sin zombis).
5. **Flujo de `call`:** `get_tool` (→ `TOOL_NOT_FOUND` con sugerencias) → `Policy::decide` → si `Confirm`, se consume cada token de `ctx.approvals` contra (`server.tool`, hash de argumentos); si ninguno sirve se crea una aprobación y se devuelve `NEEDS_APPROVAL` con el token y el mensaje fijo ``Approval required: ask the user to run `cerebro approve <token>` …``. El `consume` recibe `HubConfig.approval_mode`: `OutOfBand` (por defecto) solo acepta tokens aprobados por un humano fuera de banda; `ClientConfirm` (opt-in, más débil) acepta también pendientes → `Deny` = `DENIED` → arranque perezoso → `tools/call` con `call_timeout` (`TIMEOUT`) → `structuredContent` o, si no hay, `{text, content}`; `isError` = `TOOL_ERROR` → auditoría con argumentos redactados (claves que contienen token, password, secret o key → `"***"`) y `args_hash` blake3 del JSON canónico.
6. **Un registry nuevo por ejecución** (`build_registry`) con **todo** el catálogo, porque `execute_typescript` exige que estén todos los ids del `CodeMode`. Cada callback lleva el `CallCtx` (tokens de aprobación) y el `ExecutionScope` de esa ejecución.
7. **Runtime.** Los callbacks corren en un runtime `current_thread` efímero dentro de `spawn_blocking` (Deno no es `Send`). `Hub::new` captura `Handle::current()` y cada callback hace `handle.spawn(hub.call_inner(..)).await`, así que ni los procesos hijos ni las tareas del cliente rmcp nacen en el runtime efímero (morirían al terminar la ejecución). Verificado por `tests/sandbox.rs`.
8. **Nombres TS** con las funciones de pctx (`Case::Pascal` para el namespace, `Case::Camel` para la herramienta), sin reimplementarlas. `CallbackConfig.namespace` y `.name` llevan los nombres crudos.
9. **Colisiones.** Dos nombres crudos que dan el mismo nombre TS (`create_issue` / `createIssue`) harían fallar toda ejecución con TS2300. Se ordena por nombre crudo, gana el primero y el resto **no entra al catálogo**, con un aviso en `SyncReport.warnings`. Dos servidores con el mismo namespace PascalCase (`my-server` / `my_server`): el segundo se rechaza (`servers_failed`).
10. **Fallos aislados.** Un servidor caído queda con `status=Error` y `last_error`; los demás siguen. Una caída de transporte durante una llamada descarta la conexión (`SERVER_UNAVAILABLE`) y la siguiente llamada la relanza.

## Desviaciones menores del contrato (para validar con el coordinador)

- `SyncReport` gana el campo `warnings: Vec<String>` (aditivo).
- `ToolRecord` no tiene `available`; una herramienta descartada por colisión simplemente no se cataloga.
- `ToolRecord.ts_signature` la rellena el hub con una firma compacta derivada del `inputSchema` (`Ns.fn({ a: string, b?: number }): Promise<any>`); la salida se muestra como `any`.
- `HubConfig` gana `approval_mode` (orden del coordinador, `ApprovalMode`).
- Las llamadas a herramientas inexistentes no generan registro de auditoría (`ToolCallRecord` exige una decisión que no existe).

## MRTR (fuera de la Fase 0)

rmcp 3.11 modela las respuestas de `tools/call` con `CallToolResponse::{Complete, InputRequired, Task}`; `Peer::call_tool_once` las devuelve sin automatizar rondas y `call_tool` (en `RunningService`) las resuelve a través del `ClientHandler` local. El hub usa `call_tool_once` y trata `InputRequired` y `Task` de un downstream como `TOOL_ERROR` (no soportado). Por el lado servidor, rmcp 3.11 expone `InputRequiredResult` (`model/mrtr.rs`), que WS4 podría usar para ofrecer aprobaciones interactivas; en la Fase 0 la aprobación va por token (`NEEDS_APPROVAL` → el cliente reintenta `execute_typescript({ approvals })`).

## Consecuencias

- Toda llamada queda auditada y sujeta a política; añadir taint (M5) o MRTR es local al hub.
- Cada ejecución reconstruye el registry (coste lineal en el tamaño del catálogo, despreciable en la Fase 0).
- pctx serializa ejecuciones con un `V8_MUTEX` global; el hub en sí no impone esa limitación.
- Las anotaciones MCP las declara el servidor downstream y no son confiables: la política combina overrides de config con anotaciones y, ante conflicto (`readOnlyHint` y `destructiveHint` a la vez), gana `Confirm`.
