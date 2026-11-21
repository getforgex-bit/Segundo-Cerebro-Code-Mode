# Equipos, roles y propiedad

> Fuente de verdad de **quién hace qué**. Decisión registrada en `docs/cerebro/adr/0007-organizacion-multiagente.md`.
> La distribución es **revisable**. Cualquiera de los dos coordinadores puede proponer cambios con evidencia (Issue `type:decision`). Al cerrar cada fase se revisa obligatoriamente.

## 1. Equipos

| Equipo | Herramienta | Coordinador | Trabajadores | Responsabilidad |
|---|---|---|---|---|
| **A** | Claude Code (cloud) | **Claude Opus 5.5** (arquitecto principal e integrador) | Claude Sonnet 5.5 (1–3 activos) | Gateway / Code Mode: servidor MCP, hub, política, sandbox y parches pctx, aprobaciones (MRTR), *handles* de resultados, límites, detección de *rug-pull*, CLI, empaquetado, CI e **integración en `develop`** |
| **B** | Google Antigravity (máquina del usuario) | **Gemini 4 Neon** (coarquitecto) | Gemini 4 Hydrogen (3–4 activos, más un verificador) | **Subsistema cerebro** (memorias, `recall`/`remember`/`primer`, sincronización Markdown), **indexado y grafo de código**, **recuperación** (`cerebro_search`), **integraciones de cliente** (`cerebro_clients`) y **evals** |

**Por qué este reparto** (evidencia completa en ADR 0007):
- **Frontera real del código.** El gateway está construido y el cerebro parte de cero detrás de un trait (`Brain`), así que se trabaja en crates distintos y en paralelo.
- **Benchmarks:**
  - Terminal-Bench y FrontierCode favorecen ligeramente a Claude en lo que ya está construido e integrado.
  - CursorBench, ALE, GraphWalks y SWE-sweep favorecen a Gemini en trabajo multiarchivo ambiguo, de largo horizonte y relacional.
  - AutomationBench favorece a Gemini en integraciones.
- **Entorno.** El Equipo B corre en la máquina del usuario, donde están instalados los clientes reales (Claude Desktop, Codex, Antigravity). El Equipo A corre en contenedores cloud sin ellos.
- **Coste medido en la Fase 0** (`AGENT-EVALS.md`). Sonnet rinde mucho más por dólar que Opus en briefs acotados, y el contexto del coordinador fue ~51 % del gasto. Por eso los coordinadores deciden y delegan la mecánica.

## 2. Propiedad por rutas

| Ruta | Dueño |
|---|---|
| `crates/cerebro/`, `crates/cerebro_hub/`, `crates/cerebro_policy/`, `crates/cerebro_spike_pctx/`, `crates/pctx*` (solo parches `CEREBRO-PATCH`), `crates/cerebro_store/` (salvo `migrations/brain/` nuevas), `packages/e2e/`, `packages/fixtures/`, `.github/**` | **A** |
| `crates/cerebro_brain/`, `crates/cerebro_clients/`, `crates/cerebro_search/`, `crates/cerebro_store/migrations/brain/0002_*` y siguientes (**nunca** editar `0001_init.sql`), `packages/evals/`, `docs/cerebro/clients.md`, `.agents/**`, ADR 0006 y 0008 en adelante | **B** |
| `crates/cerebro_types/`, `coordination/CONTRACTS.md` | **Compartido**: solo vía Issue `type:contract-change` con el visto bueno de Opus y Neon; integra Opus |
| `AGENTS.md`, `CLAUDE.md`, `coordination/**` (salvo `ws/`), ADR 0001–0005 y 0007 | Opus. Neon propone cambios por PR |
| `packages/package.json`, `packages/pnpm-workspace.yaml`, `packages/pnpm-lock.yaml`, `Cargo.lock` | Compartidos: en caso de conflicto, toma la versión de `develop` y regenera |
| `README.pctx.md`, `docs/*.md` (pctx), plantillas de pctx en `.github/` | Upstream: no se tocan |

Necesitas cambiar algo ajeno → Issue al equipo dueño. El equipo B puede leer y usar `cerebro_store::Store` (incluido `Store::conn()`, `#[doc(hidden)]`) para el código de repositorio del cerebro. Si necesita una API pública mejor, la pide por Issue.

## 3. Roles iniciales (especialización inicial, no permanente)

### Equipo A

| Rol | Modelo | Responsabilidad inicial |
|---|---|---|
| Coordinador | Opus 5.5 | Arquitectura global, contratos, revisión de PRs, merge a `develop` y `STATUS.md`. Desempata **solo en integración**. Rota de sesión cada ~300k tokens o al cerrar una fase. |
| Sonnet A: motor y servidor | Sonnet 5.5 | MRTR (`CallToolResponse::InputRequired`), *handles* M6, límites desde la config y, después, cablear los handlers de `Brain` |
| Sonnet B: CLI, seguridad y empaquetado | Sonnet 5.5 | CLI `import-config`/`install`/`doctor` sobre `cerebro_clients`, detección de *rug-pull* (`schema_hash`), `cargo-dist` y wrapper npm |
| Sonnet D: QA e integración | Sonnet 5.5 | CI, E2E, preparar merges (Cargo.lock, suites completas) y regresiones |
| Sonnet C: orquestación (bajo demanda) | Sonnet 5.5 | Taint M5 junto con el Equipo B |

### Equipo B

| Rol | Modelo | Responsabilidad inicial |
|---|---|---|
| Coordinador | Gemini 4 Neon | Arquitecto del cerebro, de la recuperación y de los clientes. Copropietario de los contratos (`Brain`, API de `cerebro_clients`). **Revisor cruzado obligatorio** de los PRs críticos del Equipo A. Validación en clientes reales. Gestión del swarm. Igual a Opus en investigación. Rota de sesión cada ~250k tokens. |
| `@brain-core` | Hydrogen | `crates/cerebro_brain`: memorias, links, sesiones, sincronización MD (ADR 0004) y primer |
| `@indexer` | Hydrogen | Extractores md/config, indexado incremental, tree-sitter TS/JS/Rust/Python, SCIP y watcher |
| `@clients` | Hydrogen | `crates/cerebro_clients`: adaptadores de Claude Code, Claude Desktop, Codex y Antigravity; validación real |
| `@retrieval` | Hydrogen | `cerebro_search` (sinónimos es↔en, golden set), `packages/evals` y decisión sobre embeddings |
| `@verifier` | Hydrogen | Revisa diffs y ejecuta tests. Solo con ≥ 2 workers activos; si no, verifica Neon |

## 4. Jerarquía híbrida

| Decisión | Quién decide | Cómo |
|---|---|---|
| Visión de producto, alcance y prioridades | **Usuario** | — |
| Arquitectura transversal y contratos | Opus + Neon | Por consenso. Si no lo hay: criterios verificables y un experimento pequeño. Si el empate persiste en un tema de integración, decide Opus. |
| Arquitectura dentro del área de un equipo | Su coordinador | Con autoridad plena (Neon no necesita permiso de Opus en sus rutas) |
| Implementación | El trabajador | Dentro de su Issue y su brief |

**Desacuerdos:**
1. Se plantea problema, evidencia y alternativas.
2. La otra parte responde.
3. Se decide o se lanza un experimento.

Como máximo **2 rondas** por Issue. El resultado queda en un ADR o en un comentario de resolución. Ninguna parte ignora una instrucción explícita del usuario.

## 5. Concurrencia y cuota

- **Equipo A:** 1 Opus + 2–3 Sonnet a la vez, como máximo. En la Fase 0, con 5 sesiones simultáneas se alcanzó el límite de 5 h.
- **Equipo B:** Neon + 3–4 Hydrogen implementadores + 1 verificador, según la cuota del plan.
- No se lanzan agentes sin un Issue `status:ready` que los justifique. Se reducen cuando falta trabajo independiente.
- **Sin pagos extra.** En Claude, nada de uso extra; en Antigravity, "AI Credit Overages: Never". Salvo autorización del usuario.

## 6. Reasignación

- **Dentro de un equipo:** la decide su coordinador cuando hace falta (trabajador sin Issue listo → siguiente Issue de su equipo por prioridad, o se retira).
- **Entre equipos:** Issue `type:decision` con evidencia (≥ 3 Issues en `AGENT-EVALS.md` para cambios estables). Al aprobarse, se actualiza esta tabla por PR.
