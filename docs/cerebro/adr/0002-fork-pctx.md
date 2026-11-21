# ADR 0002 — Fork de pctx y política de parches

- **Estado:** aceptada (WS1, 2026-11-21). **Dueño:** WS1.

## Contexto
`portofcontext/pctx` (MIT) ya resuelve lo costoso del motor Code Mode: genera TypeScript desde los esquemas MCP, lo ejecuta en un sandbox Deno con type-check y enruta llamadas a callbacks de Rust. Cerebro necesita ese motor, pero con política, auditoría y aprobaciones en cada llamada, y con límites de ejecución que upstream no ofrecía (heap de V8, timeout expuesto, timeout del type-check; ver `UPSTREAM.md` §3).

## Decisión
1. Este repo es un **fork con historial** de pctx (base v1.0.0). Los crates `crates/pctx*` siguen siendo de upstream; los de Cerebro son `crates/cerebro_*` y dependen de pctx **por ruta** (`path = "../pctx_code_mode"`).
2. **Integración por callbacks, no por `add_mcp`:** el hub de Cerebro registra cada herramienta downstream como callback de pctx, así toda llamada pasa por nuestra política sin tocar el registry de upstream.
3. **Política de parches:**
   - Un parche a `crates/pctx*` requiere aprobación previa del coordinador.
   - Debe ser **mínimo y aditivo**: la API pública de pctx se conserva y los valores por defecto son los de upstream (excepción documentada: timeout del type-check de 60 s).
   - Cada cambio se marca `// CEREBRO-PATCH: <motivo>`, se lista en `UPSTREAM.md` §9 y tiene un test en `cerebro_spike_pctx`.
   - Los tests upstream de los crates tocados deben seguir en verde.
4. **Sincronización:** merge periódico de `upstream/main` según el runbook de `UPSTREAM.md` §8. Si upstream integra algo equivalente a un parche, se retira el nuestro.
5. **Sin PRs ni push a upstream** salvo aprobación expresa del usuario, caso por caso.
6. `README.md` es nuestro; el de upstream se conserva como `README.pctx.md`. Los workflows de upstream no se modifican; los nuestros son `.github/workflows/cerebro-*.yml`.

## Alternativas
- **Dependencia por crates.io/git sin fork:** descartada, porque no permite parchear límites de seguridad ni fijar versión con historial auditable.
- **Motor propio (rquickjs):** descartada, ver ADR 0003.
- **Vendoring sin historial:** se pierde la trazabilidad y hace el merge de upstream muy costoso.

## Consecuencias
- (+) Reutilizamos codegen, type-check y sandbox; el MVP baja de unas 3–4 semanas a unas 2 (estimación del plan, no medida).
- (−) Cargamos con el coste de sincronizar y con el peso de `deno_core`/V8 en compilación.
- (−) Dos versiones de `rmcp` en el grafo (1.8 de pctx y 3.11 de Cerebro); no deben cruzar tipos.
- (−) Cada parche es deuda de mantenimiento: por eso son pocos y están cubiertos por tests que fallarán si un merge los rompe.
