# Coordinación multi-sesión de la Fase 0 (temporal)

> Carpeta **temporal**. Al cerrar la Fase 0, su contenido útil se consolida en `docs/cerebro/` y la carpeta se elimina.

## Qué hay aquí

| Archivo | Para qué |
|---|---|
| [`PLAN.md`](PLAN.md) | Plan aprobado completo (v2, con el v1 en el Anexo A). Es la fuente de verdad del "qué" y el "por qué". |
| [`PROTOCOL.md`](PROTOCOL.md) | **Léelo primero.** Roles, ramas, propiedad de archivos, cómo reportar y cómo consultar al coordinador. |
| [`CONTRACTS.md`](CONTRACTS.md) | Interfaces entre crates, versiones fijadas y convenciones. Los cambios pasan por el coordinador. |
| [`STATUS.md`](STATUS.md) | Tablero de estado. Lo mantiene el coordinador. |
| [`ws/`](ws/) | Un brief por línea de trabajo (WS = *workstream*). |

## Líneas de trabajo de la Fase 0

| WS | Sesión | Rama | Alcance |
|---|---|---|---|
| — | **Coordinador** (sesión principal) | `ccr-f6dcb0c5-1kktmp` (integración) | Dirige, decide, revisa, integra y razona lo difícil. Dueño de `cerebro_types`, `coordination/` y los ADR 0001 y 0006. |
| [WS1](ws/WS1-fork.md) | fork | `ws/ws1-fork` | Análisis de pctx, `docs/cerebro/UPSTREAM.md`, ADR 0002 y 0003, CI, límites del sandbox y parches `CEREBRO-PATCH` |
| [WS2](ws/WS2-store.md) | store | `ws/ws2-store` | `cerebro_store` (SQLite) y `cerebro_search` (FTS5), ADR 0004 |
| [WS3](ws/WS3-hub-policy.md) | hub | `ws/ws3-hub` | `cerebro_hub` (rmcp 3.11), `cerebro_policy`, `packages/fixtures`, ADR 0005, modelo de amenazas |
| [WS4](ws/WS4-server-e2e.md) | server | `ws/ws4-server` | Binario `cerebro` (servidor MCP con 6 herramientas), `packages/e2e`, `meta-tools.md`, `clients.md`, `sandbox-api.d.ts` |

## Grafo de dependencias

```
cerebro_types (coordinador, ya existe)
   ├── WS2 store/search ──┐
   ├── WS3 hub/policy ────┼──► WS4 server + E2E ──► integración (coordinador)
   └── WS1 fork/CI ───────┘
```

Todas las líneas arrancan **a la vez**. Cada una programa contra los traits de `cerebro_types` con *mocks* propios hasta que la implementación real llega a la rama de integración.
