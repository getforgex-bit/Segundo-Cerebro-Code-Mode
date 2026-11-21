# ADR 0003 — Sandbox Deno heredado de pctx (QuickJS descartado)

- **Estado:** aceptada (WS1, 2026-11-21). **Dueño:** WS1.

## Contexto
El plan v1 contemplaba un sandbox propio con QuickJS (rquickjs) y oxc para transpilar. Al adoptar el fork de pctx (ADR 0002), el motor de ejecución pasa a ser el de pctx: `deno_core` 0.412 (V8) con transpilado (`pctx_deno_transpiler`) y type-check con el compilador TypeScript 5.3.3.

## Decisión
Usar el sandbox Deno de pctx y **descartar QuickJS**. Los límites y garantías, verificados con `crates/cerebro_spike_pctx`:

| Propiedad | Estado |
|---|---|
| Red | Cero: no hay `fetch`, `WebSocket` ni `Deno.connect`; solo `op_invoke` y `op_sleep`. |
| FS y módulos | Ninguno: `NoopModuleLoader`; `import()` falla. |
| Timeout | Watchdog + temporizador; 120 s por defecto, configurable (parche C2). |
| Heap de V8 | Límite configurable (parche C1); por defecto upstream: sin límite. |
| Type-check | Siempre antes de ejecutar; ~280 ms en debug; con timeout (parche C3). |
| Concurrencia | Una ejecución a la vez por proceso (`V8_MUTEX`). |
| Memoria fuera del heap | **No cubierta** (`ArrayBuffer` de 3 GiB tuvo éxito con límite de 64 MB). |

## Alternativas
- **QuickJS (rquickjs):** motor más pequeño y de arranque rápido, y más fácil de limitar en memoria. Se descarta porque obligaría a reescribir type-check, generación de tipos y puente de callbacks, que pctx ya ofrece y mantiene. **No se midió** el spike comparativo (arranque en frío, RAM, binario) que proponía el plan M1: la decisión es de coste de reescritura, no de benchmark. El trait `ExecutionEngine` del plan permitiría revisarlo.
- **Deno completo (subproceso `deno run` con permisos):** más aislamiento de proceso, pero añade un binario externo y latencia de arranque. Es la vía natural para el aislamiento por proceso de la Fase 1.

## Consecuencias
- (+) Type-check real: los errores de tipos llegan al modelo antes de ejecutar.
- (−) Binario y compilación más pesados (V8). `rusty_v8` descarga un binario precompilado en el build (funciona a través del proxy en este entorno).
- (−) Un solo V8 por proceso: sin paralelismo; en la Fase 1 se evalúa un pool de procesos worker, que además daría el límite duro de memoria que V8 no da.
- (−) Una caída fatal de V8 (p. ej. memoria externa) tumbaría el servidor: mitigación pendiente (límite de proceso o worker).
