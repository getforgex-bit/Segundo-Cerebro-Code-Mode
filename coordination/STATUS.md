# Estado de la Fase 0 (lo mantiene el coordinador)

_Última actualización: 2026-11-21 18:33 UTC: las 4 líneas relevadas a Sonnet 5.5; el coordinador sigue en Opus 5.5._

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
| WS1 fork | `session_018vGgtBtNgZCaQfUuawSJgp` (Sonnet 5.5) | en curso | relevo @ 0d3af74: spike de 15 tests (12 en verde), hallazgos en CONTRACTS | — |
| WS2 store | `session_018vGgtEA1LZaw9ewZ6jA8ar` (Sonnet 5.5) | en curso (H4 búsqueda) | H3 integrado (85c975e → 5de9ed2) | — |
| WS3 hub | `session_018vGgtDYGGyXHNEKu9BV2jF` (Sonnet 5.5) | en curso (corrección de la política → hub) | fixtures integrados; política @ 0b7e4ba; relevo @ 56ea568 | — |
| WS4 server | `session_018vGgtC1ESwsgNGC3g87aZ1` (Sonnet 5.5) | en curso (compilar y ServerHandler) | relevo @ 6e3edcb (código sin compilar) | — |

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
| 2026-11-21 | rmcp 3.11 en nuestros crates y 1.8 en los de pctx, sin tocar el workspace | Mínima divergencia del fork |

## Integraciones

| Fecha | WS | Commit | Verificación |
|---|---|---|---|
| 2026-11-21 | WS2 H1 | 3b049d6 → 74101ce | `cargo test -p cerebro_store -p cerebro_search` en verde (1 test) |
| 2026-11-21 | WS3 fixtures | c7967e7 → aac8521 | `pnpm --filter @cerebro/fixtures test`: 2/2 en verde |
| 2026-11-21 | WS2 H3 | 85c975e → 5de9ed2 | `cargo test -p cerebro_store`: 13/13; clippy limpio |

## Sesiones archivadas (Opus, primera tanda)

WS1 `session_018vGgtF7ZXHzAQv1sfgRLMG` · WS2 `session_018vGgtC4G1fD9UdCFRrMNjj` · WS3 `session_018vGgtBhBzNb8TrFHfWaVEi` · WS4 `session_018vGgtD9kvucbHZMGctThGQ`. Sus notas de relevo se pasaron en el prompt inicial de las sesiones Sonnet.
