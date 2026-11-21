# Estado de la Fase 0 (lo mantiene el coordinador)

_Última actualización: 2026-11-21, lanzamiento._

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
| WS1 fork | (pendiente de lanzar) | — | — | — |
| WS2 store | (pendiente de lanzar) | — | — | — |
| WS3 hub | (pendiente de lanzar) | — | — | — |
| WS4 server | (pendiente de lanzar) | — | — | — |

## Decisiones del coordinador

| Fecha | Decisión | Motivo |
|---|---|---|
| 2026-11-21 | Contratos compartidos en `cerebro_types` | Que las 4 líneas avancen en paralelo con mocks |
| 2026-11-21 | Config de la Fase 0 propia (`.cerebro/config.json`), no `pctx.json` | Evitar depender del parser upstream; WS1 decide si se reutiliza después |
| 2026-11-21 | rmcp 3.11 en nuestros crates y 1.8 en los de pctx, sin tocar el workspace | Mínima divergencia del fork |

## Integraciones

| Fecha | WS | Commit | Verificación |
|---|---|---|---|
| — | — | — | — |
