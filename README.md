# Segundo Cerebro · Code Mode (Cerebro)

> Fork interno de ForgeX de [**pctx**](https://github.com/portofcontext/pctx) (MIT, © Port of Context).
> El README original de pctx se conserva en [`README.pctx.md`](README.pctx.md).

**Lo que se usa hoy: [FORGEX Stack](stack/README.md).** Un comando deja cualquier proyecto listo para agentes IA (Claude Code, Claude Desktop, Codex, Antigravity):
- **pctx** (*Code Mode*): un único servidor MCP que agrega los demás sin cargar sus esquemas en el contexto. La IA escribe TypeScript que los llama y recibe solo el resultado.
- **GitNexus**: grafo de conocimiento del código (contexto, impacto, cambios).
- **Memoria del proyecto**: `@modelcontextprotocol/server-memory`, versionada en `.forgex/memory.jsonl`.

## Estado (2026-11-22)

**Cambio de rumbo:** se usa **[`stack/`](stack/README.md), FORGEX Stack**: pctx (Code Mode) + GitNexus (grafo de código) + memoria MCP, que se instala en un solo comando por proyecto. Medido: unos **736 tokens** fijos frente a unos 17,5k si GitNexus va directo.

El gateway propio **Cerebro** (`crates/cerebro*`, `coordination/`) queda **congelado** en el commit `b122858` de `develop`, con la Fase 0 completa y la CI en verde.

## Estructura

| Ruta | Origen |
|---|---|
| `stack/` | **FORGEX Stack** (propio, en uso) |
| `crates/pctx*` | upstream pctx (parches mínimos marcados con `CEREBRO-PATCH`) |
| `crates/cerebro*` | propio (ForgeX), **congelado** |
| `packages/` | propio: fixtures MCP, E2E y evals en TypeScript |
| `coordination/` | plan y coordinación multiagente de Cerebro (**congelado**) |

## Licencia

El código de pctx mantiene su licencia MIT (ver `LICENSE`). El código propio de ForgeX es de uso interno.

GitNexus (usado por `stack/`, no incluido en el repo) tiene licencia **PolyForm Noncommercial 1.0.0**: hay que migrar antes de cualquier uso comercial (ver [`stack/README.md`](stack/README.md)).
