# FORGEX Stack: Code Mode + grafo de código + memoria, listo en un comando

Kit **sin código propio de servidor** que deja cualquier proyecto preparado para que los agentes IA (Claude Code, Codex, Antigravity, Claude Desktop) trabajen con:

| Pieza | Qué aporta | Licencia |
|---|---|---|
| [**pctx**](https://github.com/portofcontext/pctx) 1.0 | *Code Mode*: **un** servidor MCP (`forgex`) con 3 herramientas. La IA escribe TypeScript que llama a las demás herramientas en un sandbox y solo vuelve el resultado. | MIT |
| [**GitNexus**](https://github.com/abhigyanpatwari/GitNexus) 1.6 | Grafo de conocimiento del código: símbolos, llamadas, flujos, impacto y diff→impacto. Indexa en local, sin LLM. | ⚠ **PolyForm Noncommercial 1.0.0** (ver abajo) |
| [`@modelcontextprotocol/server-memory`](https://github.com/modelcontextprotocol/servers/tree/main/src/memory) | Memoria del proyecto (decisiones, convenciones) en `.forgex/memory.jsonl`, que se versiona | MIT |

## Por qué así (medido en este repo, 2026-11-22, con `verify.mjs`)

- GitNexus **conectado directo** al cliente: 17 herramientas ≈ **17,5k tokens** fijos por sesión.
- **Todo detrás de pctx:** ≈ **736 tokens** fijos (3 herramientas), un **−96 %**. El catálogo completo (`list_functions`, unos 12,5k tokens) solo se paga si se pide. El bloque de `AGENTS.md` incluye una chuleta con las firmas reales para no tener que pedirlo.
- En una sola llamada `execute_typescript` la IA compone memoria y grafo (`Memory.searchNodes` + `Gitnexus.impact` + …) y recibe solo el resumen.

## Instalación (una vez por proyecto)

Requisitos: **Node ≥ 22.18** (en la rama 22) o ≥ 24.11. El proyecto debe ser un repo git.

```bash
# macOS / Linux
./stack/setup.sh /ruta/a/tu/proyecto            # opcional: --clients=claude,antigravity,codex
```
```powershell
# Windows (PowerShell)
.\stack\setup.ps1 C:\ruta\a\tu\proyecto
```

Qué hace (es idempotente y guarda una copia `.bak-*` antes de modificar cualquier archivo existente):
1. Instala globalmente, si faltan, `@portofcontext/pctx`, `gitnexus` y `@modelcontextprotocol/server-memory`.
2. `gitnexus analyze --skip-agents-md --skip-skills`. GitNexus se usa detrás de pctx, así que sus skills de MCP directo no se instalan.
3. Escribe `pctx.json` (servidores `memory` y `gitnexus`) y registra `forgex` **a nivel de proyecto**:
   - `.mcp.json` (Claude Code; al abrir el proyecto, acepta el servidor de proyecto);
   - `.agents/mcp_config.json` (Antigravity);
   - `.codex/config.toml` (Codex; solo se carga si el proyecto es *trusted*).
   - Mantiene los demás servidores que ya tuvieras.
4. Añade el bloque de instrucciones para agentes a `AGENTS.md` (entre `<!-- forgex:start/end -->`), `@AGENTS.md` a `CLAUDE.md` y `.gitnexus/` a `.gitignore`.
5. **Claude Desktop:** su configuración es global, así que el script **imprime** el fragmento y tú lo pegas en `claude_desktop_config.json` → `mcpServers`.

Comprobación: `node stack/verify.mjs /ruta/a/tu/proyecto` (debe terminar en `RESULTADO: OK`).

## Uso diario

- Los agentes siguen el bloque de `AGENTS.md`:
  - memoria al empezar;
  - `context` e `impact` antes de editar;
  - `detect-changes` antes de hacer commit;
  - guardar decisiones al terminar.
- **Reindexar tras cambios grandes:** `gitnexus analyze --skip-agents-md --skip-skills` (es incremental; añade `--watch` para mantenerlo vivo).
- **Añadir más MCP** (GitHub, Slack…) detrás de pctx: añade una entrada en `pctx.json` → `servers`, con `{ "name": "github", "url": "https://…" }` o `{ "name": "x", "command": "…", "args": [...] }`. Siguen sin coste fijo de contexto. Formato completo en [`docs/config.md`](../docs/config.md).
  - En Windows, los CLI de npm se lanzan con `"command": "cmd", "args": ["/c", "<bin>", …]`, como ya hace `configure.mjs`.
  - Usa binarios instalados y no `npx`: pctx da a cada servidor **30 s** para registrarse, y `npx` puede pasarse de ese tiempo.

## ⚠ Licencia de GitNexus: antes de cualquier uso comercial

GitNexus se distribuye con **PolyForm Noncommercial 1.0.0**. Su uso comercial, incluido el trabajo para clientes o productos de ForgeX, requiere una licencia comercial de sus autores (akonlabs.com). Decisión del 2026-11-22: **usarlo ahora y migrar antes de cualquier uso comercial** a una alternativa con licencia permisiva, verificando la licencia exacta en ese momento:
- **CodeGraphContext** (grafo de código + MCP, MIT según directorios de terceros);
- **Serena** (navegación semántica por LSP).

La migración consiste en cambiar la entrada `gitnexus` de `pctx.json` y la chuleta del bloque de `AGENTS.md`.

## Límites conocidos

- **Seguridad:** pctx ejecuta cualquier herramienta que la IA invoque desde `execute_typescript`, **sin aprobación por herramienta**. No añadas servidores con acciones destructivas o irreversibles sin pensarlo. Si te hacen falta, conéctalos directamente al cliente para que este pida confirmación.
- **No verificado en este entorno** (pruébalo en tu máquina):
  - los clientes reales (Claude Code, Codex, Antigravity, Claude Desktop);
  - `setup.ps1` en Windows y el lanzamiento con `cmd /c`.
