# Coordinación multiagente

> Coordinación permanente de los equipos de FORGEX Brain. Empezó como el espacio temporal de la Fase 0 (sesiones WS1–WS4) y ahora coordina a **dos equipos**: A en Claude Code y B en Antigravity.
> Punto de entrada general: [`../AGENTS.md`](../AGENTS.md).

| Archivo | Para qué |
|---|---|
| [`TEAMS.md`](TEAMS.md) | **Equipos, roles, propiedad por rutas, jerarquía y concurrencia** |
| [`PROTOCOL.md`](PROTOCOL.md) | §1–7 mensajes dentro del Equipo A · §8 Issues y PRs entre equipos · §9 relevo · §10 ramas |
| [`CONTRACTS.md`](CONTRACTS.md) | Interfaces entre crates y proceso de cambio (§7). Código fuente: `crates/cerebro_types` |
| [`STATUS.md`](STATUS.md) | Estado, decisiones del coordinador e integraciones |
| [`AGENT-EVALS.md`](AGENT-EVALS.md) | Evaluación empírica de modelos y registro de métricas |
| [`PLAN.md`](PLAN.md) | Plan de producto (v2) y roadmap. Lee solo la sección que necesites |
| [`teams/NEON-BOOTSTRAP.md`](teams/NEON-BOOTSTRAP.md) | Prompt de arranque del coordinador del Equipo B |
| [`ws/`](ws/) | Briefs de la Fase 0 (WS1–WS4), **cerrados**. Se conservan como referencia |

## Fases

| Fase | Equipo | Estado |
|---|---|---|
| 0: fork, contratos, store, búsqueda, hub, política, servidor y E2E | A | Cierre en curso (ver `STATUS.md`) |
| 1: MVP del gateway (MRTR, handles, CLI de clientes, *rug-pull*, empaquetado) | A | Issues `team:a` |
| 2: segundo cerebro (memorias, indexado, grafo, recuperación, clientes, evals) | B | Issues `team:b` |

Las fases 1 y 2 avanzan **en paralelo** cuando se cumplen las condiciones de arranque de `TEAMS.md`, recogidas en ADR 0007 §"Arranque".
