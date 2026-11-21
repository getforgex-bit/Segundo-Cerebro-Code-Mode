# Segundo Cerebro · Code Mode (Cerebro)

> Fork interno de ForgeX de [**pctx**](https://github.com/portofcontext/pctx) (MIT, © Port of Context).
> El README original de pctx se conserva en [`README.pctx.md`](README.pctx.md).

**Cerebro** es un segundo cerebro por proyecto más un gateway MCP al estilo *Code Mode*:
- La IA (Claude Code, Claude Desktop, Codex, Antigravity…) conecta **un único servidor MCP**.
- Ese servidor agrega N servidores MCP **sin cargar sus esquemas en el contexto**.
- La IA **busca** la herramienta, **escribe TypeScript** que la usa en un sandbox Deno (heredado de pctx) y recibe **solo el resultado final**.
- Encima, un cerebro del proyecto en SQLite (memorias ↔ código ↔ herramientas ↔ skills) que se consulta por búsqueda y se comparte con el equipo vía Markdown en git.

## Estado

Fase 0 (fork e integración) en curso, con varias sesiones en paralelo.
- Plan y coordinación: [`coordination/`](coordination/README.md)
- Documentación propia: [`docs/cerebro/`](docs/cerebro/)

## Estructura

| Ruta | Origen |
|---|---|
| `crates/pctx*` | upstream pctx (parches mínimos marcados con `CEREBRO-PATCH`) |
| `crates/cerebro*` | propio (ForgeX) |
| `packages/` | propio: fixtures MCP, E2E y evals en TypeScript |
| `coordination/` | plan dividido y protocolo multi-sesión (**temporal**, Fase 0) |

## Licencia

El código de pctx mantiene su licencia MIT (ver `LICENSE`). El código propio de ForgeX es de uso interno.
