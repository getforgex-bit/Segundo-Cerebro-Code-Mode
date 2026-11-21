# Estado del proyecto (lo mantiene el coordinador del Equipo A)

_Última actualización: 2026-11-21 ~23:00 UTC: cierre de la Fase 0 en curso y organización de dos equipos preparada (ADR 0007)._

## Equipos (detalle en `TEAMS.md`)

| Equipo | Coordinador | Estado |
|---|---|---|
| A · Claude Code | Opus 5.5 · `session_018vGgtDkbBcKozrRZMWdwQz` | **Fase 0 cerrada**; siguiente: Issues `team:a` #7–#11 (Fase 1) |
| B · Antigravity | Gemini 4 Neon | **Condiciones de arranque cumplidas**: falta que el usuario pegue `teams/NEON-BOOTSTRAP.md` en Antigravity |

### Issues abiertos clave

| Issue | Equipo | Título | Estado |
|---|---|---|---|
| #1 | A+B | Coordinación A↔B (raíz) | abierto permanente |
| #2 | B | B1 · contrato `Brain` + ADR 0008 | ready |
| #3 | B | B2 · `cerebro_brain` memorias (depende de #2) | ready |
| #4 | B | B3 · indexado v1 | ready |
| #5 | B | B4 · `cerebro_clients` (bloquea a #9) | ready |
| #6 | B | B5 · sinónimos + golden set + evals | ready |
| #7 | A | A1 · MRTR | ready |
| #8 | A | A2 · handles M6 | ready |
| #9 | A | A3 · CLI de clientes | blocked (#5) |
| #10 | A | A4 · *rug-pull* | ready |
| #11 | A | A5 · empaquetado | ready |

## Base común (coordinador) en `ccr-f6dcb0c5-1kktmp`

- [x] Fork de pctx importado con historial (`git merge --allow-unrelated-histories upstream/main`, upstream @ `499379d`).
- [x] `README.md` propio con atribución; el original se conserva en `README.pctx.md`.
- [x] `cargo build -p pctx_code_mode` en verde en el entorno (2 min 33 s en frío; la descarga de `rusty_v8` funciona a través del proxy).
- [x] `crates/cerebro_types`: contratos con 2 tests en verde y clippy limpio.
- [x] Workspace pnpm en `packages/`.
- [x] Plan dividido en `coordination/`.

## Líneas de trabajo de la Fase 0 (Equipo A)

| WS | Sesión | Estado | Último hito | Bloqueos |
|---|---|---|---|---|
| WS1 fork | `session_018vGgtBtNgZCaQfUuawSJgp` (Sonnet 5.5) | **completo** | final 1cc7838: UPSTREAM, ADR 0002/0003, CI; parche C1–C3 | — |
| WS2 store | `session_018vGgtEA1LZaw9ewZ6jA8ar` (Sonnet 5.5) | **completo** (en reposo) | permisos + .gitignore + threat-model (8320e98) | — |
| WS3 hub | `session_018vGgtDYGGyXHNEKu9BV2jF` (Sonnet 5.5) | **completo** | hub v2 1593537 (policy 9 + hub 19 tests) | — |
| WS4 server | `session_018vGgtC1ESwsgNGC3g87aZ1` (Sonnet 5.5) | **completo** | final bf77252: hub real por defecto, límites (timeout + heap 256 MB) y E2E 10/10 sin mock; tools/list = 2096 B ≈ 524 tokens | — |

## Decisiones del coordinador

| Fecha | Decisión | Motivo |
|---|---|---|
| 2026-11-21 | Contratos compartidos en `cerebro_types` | Que las 4 líneas avancen en paralelo con mocks |
| 2026-11-21 | Config de la Fase 0 propia (`.cerebro/config.json`), no `pctx.json` | Evitar depender del parser upstream; WS1 decide si se reutiliza después |
| 2026-11-21 | `packages/.gitignore` (node_modules, */dist) y `pnpm-lock.yaml` tratado como Cargo.lock | Propuesta de WS3 |
| 2026-11-21 | Se acepta la subida de thiserror 2.0.17→2.0.21 en Cargo.lock por rusqlite 0.41 | Parche semver, sin riesgo para upstream |
| 2026-11-21 | Sesiones de trabajo en **Sonnet 5.5** y coordinador en **Opus 5.5** (petición del usuario); relevo de las 4 sesiones | — |
| 2026-11-21 | `format_hits` muestra `ts_namespace.ts_name` (CONTRACTS §4) | Consulta de WS2: que la IA use el mismo nombre en búsqueda, detalles y código |
| 2026-11-21 | Política: readOnly y destructive a la vez → Confirm (CONTRACTS §2) | Seguridad: ante anotaciones contradictorias gana lo más restrictivo |
| 2026-11-21 | Namespaces en PascalCase según pctx; `ts_namespace` y `ts_name` se calculan con las funciones de `pctx_codegen` | Hallazgo de WS4: coherencia entre búsqueda, detalles y código |
| 2026-11-21 | Callbacks del hub vía `main_handle.spawn(...)` | Los callbacks corren en un runtime efímero (`spawn_blocking`) |
| 2026-11-21 | Aprobaciones por token en la Fase 0; MRTR (`CallToolResponse::InputRequired`) en la Fase 1 | Viable en rmcp 3.11, pero sin probar con clientes |
| 2026-11-21 | **Parche aprobado** a pctx (WS1): exponer en `CodeMode` el timeout del executor sin romper la API | Hoy `tokio::time::timeout` no mata el hilo de Deno |
| 2026-11-21 | **Parches aprobados** a pctx (WS1, `CEREBRO-PATCH`, aditivos y sin romper la API): (C1) límite de heap V8 (`heap_limits` + `near_heap_limit_callback` → `terminate_execution`) en el executor y el type-check; (C2) `CodeMode::execute_typescript_with_options` con timeout y heap; (C3) watchdog con timeout para el type-check | Hoy una bomba de memoria tumba el proceso host y el type-check no tiene límite |
| 2026-11-21 | Errores de callback: stderr y, si no aparecen, la traza (C5, sin parche) | `console.error` previo tapa el error |
| 2026-11-21 | Hub con una sola conexión rmcp por servidor (multiplexada) | Simplicidad; pool solo si se mide la necesidad |
| 2026-11-21 | Colisiones de nombre TS: deduplicar en el hub y rechazar namespaces que coincidan en la config | pctx falla con TS2300 en toda ejecución |
| 2026-11-21 | Type-check de unos 280 ms por ejecución sin caché y una sola ejecución a la vez por proceso (V8_MUTEX): se aceptan en la Fase 0 | Optimizar en la Fase 1 con datos |
| 2026-11-21 | `ToolHit.ts_namespace` (cerebro_types @ a2554da) | Consulta de WS2: `format_hits` necesita el namespace PascalCase del catálogo |
| 2026-11-21 | `docs/cerebro/threat-model.md` pasa de WS3 a WS2 | Descargar a WS3, que lleva la ruta crítica (el hub) |
| 2026-11-21 | **Seguridad:** las aprobaciones exigen aprobación humana fuera de banda por defecto (`ApprovalMode::OutOfBand`, `cerebro approve` solo con TTY); `ClientConfirm` como opt-in; MRTR pasa a ser la prioridad 1 de la Fase 1 | threat-model de WS2: la IA podía reenviar el token sin humano |
| 2026-11-21 | Backlog de la Fase 1: detectar cambios de `schema_hash` en herramientas existentes (*rug pull*) → `confirm` hasta revisión | threat-model de WS2 |
| 2026-11-21 | DoD de clippy: `cargo clippy -p <crate> --all-targets --no-deps -- -D warnings` | Sin `--no-deps`, clippy analiza también los crates de pctx (con avisos propios) |
| 2026-11-21 | rmcp 3.11 en nuestros crates y 1.8 en los de pctx, sin tocar el workspace | Mínima divergencia del fork |

## Integraciones

- **2026-11-21 · cierre de la Fase 0:** WS3 (f493d05), parche WS1 (4815294), WS1 final, WS4 final y CI con E2E real obligatorio. Verificado por el coordinador:
  - store 16, search 11, policy 9, hub 19, types 2 y cerebro 15 tests;
  - clippy `--no-deps` limpio;
  - **E2E 10/10 contra el binario real**;
  - la CI falla, en lugar de omitir, si el E2E no puede ejecutarse.

  Se crea **`develop`**.

| Fecha | WS | Commit | Verificación |
|---|---|---|---|
| 2026-11-21 | WS2 H1 | 3b049d6 → 74101ce | `cargo test -p cerebro_store -p cerebro_search` en verde (1 test) |
| 2026-11-21 | WS3 fixtures | c7967e7 → aac8521 | `pnpm --filter @cerebro/fixtures test`: 2/2 en verde |
| 2026-11-21 | WS2 H3 | 85c975e → 5de9ed2 | `cargo test -p cerebro_store`: 13/13; clippy limpio |
| 2026-11-21 | WS2 permisos y .gitignore | 8320e98 | store 16 + search 11; clippy --no-deps limpio |
| 2026-11-21 | WS2 threat-model | 158726a | `cargo test -p cerebro_search`: 11; clippy limpio |
| 2026-11-21 | Coordinador: `ApprovalMode` + `list_pending` | ver el siguiente commit | `cargo test -p cerebro_store`: 15; clippy limpio |
| 2026-11-21 | WS2 H4+H5 | afca2ed | `cargo test -p cerebro_store -p cerebro_search`: 13 + 10; clippy limpio |

## Sesiones archivadas (Opus, primera tanda)

WS1 `session_018vGgtF7ZXHzAQv1sfgRLMG` · WS2 `session_018vGgtC4G1fD9UdCFRrMNjj` · WS3 `session_018vGgtBhBzNb8TrFHfWaVEi` · WS4 `session_018vGgtD9kvucbHZMGctThGQ`. Sus notas de relevo se pasaron en el prompt inicial de las sesiones Sonnet.
