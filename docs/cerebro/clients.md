# Conectar clientes a `cerebro`

`cerebro` es un servidor MCP por **stdio**: el cliente lo lanza como proceso hijo.

```
cerebro serve --stdio [--project <dir>] [--config <ruta>]
```

- `--project`: directorio del proyecto (por defecto `.`). Usa `<project>/.cerebro/config.json`, `brain.db` y `hub.db`.
- Los logs van a **stderr** (stdout es el canal MCP). Nivel con `RUST_LOG` (por defecto `cerebro=info`).
- `cerebro sync [--project]` sincroniza el catálogo e imprime el resumen, útil para comprobar la config sin cliente.

## Config del proyecto (`.cerebro/config.json`)

```json
{
  "servers": [{ "name": "echo", "command": "node", "args": ["packages/fixtures/dist/echo.js"] }],
  "policy": { "default_unknown": "confirm", "overrides": { "echo.*": "allow" } },
  "limits": { "timeout_ms": 10000, "max_result_bytes": 20000, "heap_limit_mb": 256 },
  "approvals": { "mode": "out_of_band" }
}
```

> **Aviso (ahorro de tokens):** para que haya ahorro, el cliente debe **dejar de cargar los servidores MCP originales**. Si siguen registrados en el cliente además de `cerebro`, sus herramientas siguen ocupando contexto y solo se añaden las 6 de `cerebro`. Mueve esos servidores a `servers` de `.cerebro/config.json` y quítalos del cliente.

> **Aviso de seguridad (allowlists):** si el cliente tiene una allowlist/auto-aprobación de herramientas, **no aprobar automáticamente `execute_typescript`** a ciegas: ejecuta código con acceso a todas las herramientas downstream. La política de Cerebro (`allow`/`confirm`/`deny`) y las aprobaciones protegen las llamadas destructivas, pero un cliente que auto-apruebe todo y un modo `client_confirm` dejan la confirmación en manos del propio cliente (ver más abajo). Allowlistea solo `search_functions`, `get_function_details`, `cerebro_recall` y `cerebro_primer`.

## Claude Code

```
claude mcp add cerebro -- cerebro serve --stdio --project /ruta/al/proyecto
```

o en `.mcp.json` del proyecto:

```json
{ "mcpServers": { "cerebro": { "command": "cerebro", "args": ["serve", "--stdio", "--project", "."] } } }
```

## Claude Desktop (`claude_desktop_config.json`)

```json
{ "mcpServers": { "cerebro": { "command": "/ruta/a/cerebro", "args": ["serve", "--stdio", "--project", "/ruta/al/proyecto"] } } }
```

Usa rutas absolutas: Claude Desktop no hereda el `PATH` de tu shell.

## Codex (`~/.codex/config.toml`)

```toml
[mcp_servers.cerebro]
command = "cerebro"
args = ["serve", "--stdio", "--project", "/ruta/al/proyecto"]
```

## Antigravity 2.0

`~/.gemini/config/mcp_config.json` (global) o `.agents/mcp_config.json` (proyecto):

```json
{ "mcpServers": { "cerebro": { "command": "cerebro", "args": ["serve", "--stdio", "--project", "."] } } }
```

> Las rutas y el formato de estos cuatro clientes provienen del brief de WS4 y **no se han verificado en esta sesión contra la documentación vigente de cada cliente**; confírmalo antes de repartir la guía.

## Aprobaciones: cómo aprueba el usuario

Cuando la política decide `confirm` para una llamada (p. ej. una herramienta con `destructiveHint`), `execute_typescript` devuelve `error.code = "NEEDS_APPROVAL"` con un `approval_token` y el mensaje:

> Approval required: ask the user to run `cerebro approve <token>` in an interactive terminal, then retry with approvals: ["<token>"]

La IA **ve** el token, así que si bastara con reenviarlo, una IA manipulada por inyección de prompts se autoaprobaría. Por eso hay dos modos (`approvals.mode`):

- **`out_of_band` (por defecto):** el token solo vale si un humano lo aprobó antes.
  1. `cerebro approvals [--project <dir>]` lista las pendientes (token, herramienta, expiración, motivo).
  2. `cerebro approve <token> [--project <dir>]` muestra herramienta, motivo y expiración y pide `y/N`.
  3. La IA reintenta con `approvals: ["<token>"]`. El token es de un solo uso y caduca (600 s por defecto).
  - `cerebro approve` **se niega a ejecutarse si stdin o stdout no son una TTY** (y no hay `--yes` ni variable de entorno para saltarlo). Esto impide que un agente lo ejecute con una herramienta bash sin TTY, pero **no es infalible**: un agente con acceso a una terminal interactiva (o que pueda editar `.cerebro/brain.db`) podría aprobarse a sí mismo. Se apoya en los permisos de bash del cliente. La solución estructural (confirmación en la UI del cliente vía MRTR) está planificada para la Fase 1.
- **`client_confirm` (más débil, opt-in):** también acepta tokens `pending`. Confía en que el cliente pida confirmación humana para cada `execute_typescript`. Úsalo solo si tu cliente lo hace de forma fiable.
