# AGENTS.md: FORGEX Brain (nombre en código: Cerebro)

> El punto de entrada para **cualquier agente** (Claude Code, Antigravity u otros). Antigravity lo carga automáticamente; Claude Code, a través de `CLAUDE.md`.
> Es un índice: **lee solo lo que tu tarea necesite**. Idioma del proyecto: **español** (los identificadores de código, en inglés).

## Qué es

1. **Gateway MCP estilo *Code Mode*.** El cliente IA conecta **un** servidor MCP (`crates/cerebro`) que agrega N servidores sin cargar sus esquemas en el contexto. La IA:
   - busca con `search_functions`;
   - lee la firma con `get_function_details`;
   - escribe TypeScript que se ejecuta en un sandbox Deno (`execute_typescript`) y recibe **solo** el resultado final.
2. **Segundo cerebro por proyecto.** Memorias, documentos y código interconectados en SQLite, consultables por búsqueda (`cerebro_recall`, `cerebro_remember`, `cerebro_primer`) y compartidos con el equipo vía Markdown en git.

**Stack:** Rust, TypeScript y SQLite. El repo es un **fork con historial de [pctx](https://github.com/portofcontext/pctx) (MIT)**:
- `crates/pctx*` es upstream, con parches mínimos marcados `CEREBRO-PATCH` (lista en `docs/cerebro/UPSTREAM.md`);
- `crates/cerebro*` es nuestro.

## Mapa de documentación (una fuente de verdad por tema)

| Tema | Fuente |
|---|---|
| Plan de producto y roadmap (v2 + v1 en el Anexo A) | `coordination/PLAN.md` (lee solo la sección que cite tu Issue) |
| **Equipos, roles, propiedad de rutas y jerarquía** | `coordination/TEAMS.md` |
| Protocolo: mensajes del Equipo A (§1–7), Issues/PRs entre equipos (§8), relevo (§9), ramas (§10) | `coordination/PROTOCOL.md` |
| Contratos entre crates y su proceso de cambio | `coordination/CONTRACTS.md` + `crates/cerebro_types/src/lib.rs` |
| Estado actual e integraciones | `coordination/STATUS.md` |
| Evaluación empírica de modelos | `coordination/AGENT-EVALS.md` |
| Decisiones duraderas | `docs/cerebro/adr/` |
| Esquema SQLite | `docs/cerebro/schema.md` + `crates/cerebro_store/migrations/` |
| Modelo de amenazas | `docs/cerebro/threat-model.md` |
| Fork y sincronización con pctx | `docs/cerebro/UPSTREAM.md` |
| Docs originales de pctx (no se editan) | `README.pctx.md`, `docs/*.md` |

## Equipos (detalle en `coordination/TEAMS.md`)

- **Equipo A, Claude Code** (Opus 5.5 coordina a Sonnet 5.5): gateway/Code Mode (servidor, hub, política, sandbox y parches pctx, aprobaciones, CI e integración). **Opus integra y es el único que hace merge a `develop`.**
- **Equipo B, Antigravity** (Gemini 4 Neon coordina a Gemini 4 Hydrogen): subsistema cerebro, indexado y grafo de código, recuperación (`cerebro_search`), integraciones de cliente (`cerebro_clients`) y evals.
- **Usuario:** visión de producto, alcance, prioridades y decisiones de alto impacto.

## Reglas duras (para todos)

1. **Ramas:**
   - la troncal es `develop`; `main` es solo para versiones estables aprobadas por el usuario;
   - se trabaja en `a/<issue>-<slug>` (Equipo A) o `b/<issue>-<slug>` (Equipo B);
   - **prohibido:** `--force`, reescribir historia publicada, push a `main` y hacer merge a `develop` sin ser Opus.
2. **Rama Astra:** no toques, fusiones, rebases ni elimines ninguna rama `astra*` (hoy no existe).
3. **Upstream pctx:** no hagas push al remoto `upstream` ni abras PRs o issues en `portofcontext/pctx` sin autorización del usuario.
4. **Propiedad:** edita solo las rutas de tu equipo (`coordination/TEAMS.md` §2). Lo compartido (`crates/cerebro_types`, `coordination/CONTRACTS.md`) cambia **solo** vía Issue `type:contract-change` con el visto bueno de ambos coordinadores.
5. **Cuota:** sin pagos ni créditos extra ("AI Credit Overages: Never" y nada de uso extra en Claude) salvo autorización. Lanza el **mínimo** de agentes que la tarea necesite.
6. **Verdad:** no inventes archivos, capacidades, permisos ni resultados. Distingue *propuesta*, *hecho* y *verificado*, y cita el comando que ejecutaste.
7. **Seguridad:** nunca pongas secretos en el repo, en Issues ni en logs. Nada que reduzca las aprobaciones fuera de banda (CONTRACTS §2, "Aprobaciones") sin un ADR.

## Cómo empezar una tarea

1. Busca tu Issue (`team:a`/`team:b` + `status:ready`) y lee **solo** lo que cite.
2. Crea la rama desde `develop` y marca el Issue `status:in-progress`.
3. Compila y prueba con `-p`:
   - `cargo test -p <crate>`;
   - `cargo clippy -p <crate> --all-targets --no-deps -- -D warnings`;
   - `cargo fmt -p <crate> --check`.

   Nunca hagas el build del workspace entero, porque descarga dependencias pesadas.
4. TS: `pnpm -C packages install && pnpm -C packages -r build && pnpm -C packages -r test`.
5. PR a `develop` (formato en PROTOCOL §8) → CI en verde → revisión (cruzada si el cambio es crítico) → merge (lo hace Opus).
6. Antes de que se te acaben la cuota o el contexto, haz el **relevo** (PROTOCOL §9).

## Prerrequisitos

- Rust stable (`rust-toolchain.toml`), Node ≥ 22 y pnpm 10.
- La primera compilación de `pctx_code_mode` descarga `rusty_v8` (unos 2–3 min).
- Los fixtures MCP están en `packages/fixtures/dist/` y se generan con `pnpm -C packages -r build`.
