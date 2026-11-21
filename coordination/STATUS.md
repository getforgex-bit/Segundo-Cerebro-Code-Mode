# Estado de la Fase 0 (lo mantiene el coordinador)

_Última actualización: 2026-11-21 18:30 UTC._

## Base común (coordinador) en `ccr-f6dcb0c5-1kktmp`

- [x] Fork de pctx importado con historial (`git merge --allow-unrelated-histories upstream/main`, upstream @ `499379d`).
- [x] `README.md` propio con atribución; el original se conserva en `README.pctx.md`.
- [x] `cargo build -p pctx_code_mode` en verde en el entorno (2 min 33 s en frío; la descarga de `rusty_v8` funciona a través del proxy).
- [x] `crates/cerebro_types`: contratos con 2 tests en verde y clippy limpio.
- [x] Workspace pnpm en `packages/`.
- [x] Plan dividido en `coordination/`.

## Líneas de trabajo

| WS | Sesión | Estado | Último hito | Bloqueos |
|---|---|---|---|---|
| WS1 fork | `session_018vGgtF7ZXHzAQv1sfgRLMG` | en curso | lanzada 18:24 UTC | — |
| WS2 store | `session_018vGgtC4G1fD9UdCFRrMNjj` | en curso (H2 migraciones) | H1 API integrada (3b049d6) | — |
| WS3 hub | `session_018vGgtBhBzNb8TrFHfWaVEi` | en curso (H2 política) | fixtures integrados (c7967e7) | — |
| WS4 server | `session_018vGgtD9kvucbHZMGctThGQ` | en curso | lanzada 18:24 UTC | — |

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
| 2026-11-21 | rmcp 3.11 en nuestros crates y 1.8 en los de pctx, sin tocar el workspace | Mínima divergencia del fork |

## Integraciones

| Fecha | WS | Commit | Verificación |
|---|---|---|---|
| 2026-11-21 | WS2 H1 | 3b049d6 → 74101ce | `cargo test -p cerebro_store -p cerebro_search` en verde (1 test) |
| 2026-11-21 | WS3 fixtures | c7967e7 → aac8521 | `pnpm --filter @cerebro/fixtures test`: 2/2 en verde |
