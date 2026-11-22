# FORGEX Stack: Code Mode + grafo de código + memoria, listo en un comando

Kit **sin código propio de servidor** que deja cualquier proyecto preparado para que los agentes IA (Claude Code, Codex, Antigravity, Claude Desktop) trabajen con:

| Pieza | Qué aporta | Licencia |
|---|---|---|
| [**pctx**](https://github.com/portofcontext/pctx) 1.0 | *Code Mode*: **un** servidor MCP (`forgex`) con 3 herramientas. La IA escribe TypeScript que llama a las demás herramientas en un sandbox y solo vuelve el resultado. | MIT |
| [**GitNexus**](https://github.com/abhigyanpatwari/GitNexus) 1.6 | Grafo de conocimiento del código: símbolos, llamadas, flujos, impacto y diff→impacto. Indexa en local, sin LLM. | ⚠ **PolyForm Noncommercial 1.0.0** (ver abajo) |
| [`@modelcontextprotocol/server-memory`](https://github.com/modelcontextprotocol/servers/tree/main/src/memory) | Memoria del proyecto (decisiones, convenciones) en `.forgex/memory.jsonl`, que se versiona | MIT |

## Inicio rápido (5 minutos)

Necesitas **git** y **Node.js LTS** (^22.18 o ≥ 24.11; [nodejs.org](https://nodejs.org)). Tu proyecto debe ser un repo git.

**Windows (PowerShell):**
```powershell
git clone --depth 1 -b develop https://github.com/getforgex-bit/Segundo-Cerebro-Code-Mode.git $HOME\forgex-stack
powershell -ExecutionPolicy Bypass -File $HOME\forgex-stack\stack\setup.ps1 C:\ruta\a\tu\proyecto
```

**macOS / Linux:**
```bash
git clone --depth 1 -b develop https://github.com/getforgex-bit/Segundo-Cerebro-Code-Mode.git ~/forgex-stack
~/forgex-stack/stack/setup.sh /ruta/a/tu/proyecto
```

Tiene que terminar en **`RESULTADO: OK`**. Después:
1. Abre el proyecto en **Claude Code** (`cd` al proyecto y `claude`) y **acepta** el servidor de proyecto `forgex` cuando te lo pregunte. Con `/mcp` puedes ver que está conectado.
2. Prueba esta petición: *"Usa forgex: busca en la memoria del proyecto y, con GitNexus, dime qué se rompe si cambio la función `<nombre>`."*
3. **Codex:** marca el proyecto como *trusted*. **Antigravity:** lee `.agents/mcp_config.json`. **Claude Desktop:** pega el fragmento que imprime el script.

Para otro proyecto, repite solo la segunda línea con la ruta nueva. Para actualizar el kit: `git -C $HOME\forgex-stack pull` (Windows) o `git -C ~/forgex-stack pull`.

## Por qué así (medido en este repo, 2026-11-22, con `verify.mjs`)

- GitNexus **conectado directo** al cliente: 17 herramientas ≈ **17,5k tokens** fijos por sesión.
- **Todo detrás de pctx:** ≈ **736 tokens** fijos (3 herramientas), un **−96 %**. El catálogo completo (`list_functions`, unos 12,5k tokens) solo se paga si se pide. El bloque de `AGENTS.md` incluye una chuleta con las firmas reales para no tener que pedirlo.
- En una sola llamada `execute_typescript` la IA compone memoria y grafo (`Memory.searchNodes` + `Gitnexus.impact` + …) y recibe solo el resumen.

## Instalación en detalle

Opciones: `--clients=claude,codex` en `setup.sh` o `-Clients claude,codex` en `setup.ps1` para elegir clientes, y `-SkipVerify` en `setup.ps1` para saltarse la verificación.

Qué hace (es idempotente y guarda una copia `.bak-*` antes de modificar cualquier archivo existente):
1. Instala globalmente, si faltan, `@portofcontext/pctx`, `gitnexus` y `@modelcontextprotocol/server-memory`.
2. `gitnexus analyze --skip-agents-md --skip-skills`. GitNexus se usa detrás de pctx, así que sus skills de MCP directo no se instalan.
3. Escribe `pctx.json` (servidores `memory` y `gitnexus`) y registra `forgex` **a nivel de proyecto**:
   - `.mcp.json` (Claude Code; al abrir el proyecto, acepta el servidor de proyecto);
   - `.agents/mcp_config.json` (Antigravity);
   - `.codex/config.toml` (Codex; solo se carga si el proyecto es *trusted*).
   - Mantiene los demás servidores que ya tuvieras.
4. Añade el bloque de instrucciones para agentes a `AGENTS.md` (entre `<!-- forgex:start/end -->`), `@AGENTS.md` a `CLAUDE.md` y `.gitnexus/` a `.gitignore`.
5. **Claude Desktop:** su configuración es global, así que el script **imprime** el fragmento y tú lo pegas en `claude_desktop_config.json` → `mcpServers` (en Windows: `%APPDATA%\Claude\claude_desktop_config.json`).
6. **Verifica** con `verify.mjs`: arranca pctx, crea, lee y borra una memoria de prueba, consulta GitNexus a través de Code Mode y mide los tokens. Puedes repetirlo cuando quieras con `node <kit>/stack/verify.mjs <proyecto>`.

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

- **Rutas absolutas:** `pctx.json`, `.mcp.json`, `.agents/mcp_config.json` y `.codex/config.toml` llevan rutas de **tu** máquina. Si trabajas en equipo, que cada persona ejecute el setup en su máquina y no versionéis esos archivos. La memoria (`.forgex/memory.jsonl`) y `AGENTS.md` sí se comparten.

- **Seguridad:** pctx ejecuta cualquier herramienta que la IA invoque desde `execute_typescript`, **sin aprobación por herramienta**. No añadas servidores con acciones destructivas o irreversibles sin pensarlo. Si te hacen falta, conéctalos directamente al cliente para que este pida confirmación.
- **Verificado** (en Linux): `setup.sh` de punta a punta desde una copia limpia, con `RESULTADO: OK`, y que repetirlo no cambia nada.
- **No verificado** (pruébalo en tu máquina):
  - los clientes reales (Claude Code, Codex, Antigravity, Claude Desktop);
  - `setup.ps1` en Windows: está revisado para PowerShell 5.1 (comillas, códigos de salida y BOM), pero no se ha ejecutado;
  - el lanzamiento con `cmd /c`.
- **Si `setup.ps1` falla en Windows:** el mensaje dice qué paso ha fallado. Lo más habitual:
  - Node antiguo;
  - `npm prefix -g` fuera del PATH (añádelo y abre otra terminal);
  - la política de ejecución (usa la línea `powershell -ExecutionPolicy Bypass -File …`).
