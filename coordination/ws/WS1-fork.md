# WS1: Fork, sandbox y CI

**Rama:** `ws/ws1-fork` · **Coordinador:** `session_018vGgtDkbBcKozrRZMWdwQz` · **Lee antes:** `../PROTOCOL.md`, `../CONTRACTS.md`, y `../PLAN.md` §V2.1, V2.5, V2.7 (0a, 0b, 0h) y V2.9.

## Objetivo

Que el resto del equipo pueda **apoyarse en pctx con certezas y no con suposiciones**: documentar cómo se extiende, fijar los límites del sandbox, dejar la CI verde y definir cómo se sincroniza el fork con upstream.

## Contexto

- El repo ya es un fork de `portofcontext/pctx` con su historial. El remoto `upstream` existe en la sesión del coordinador; en tu sesión, añádelo tú con `git remote add upstream https://github.com/portofcontext/pctx.git`. **Nunca hagas push a `upstream`.**
- `cargo build -p pctx_code_mode` compila en este entorno, y el binario precompilado de `rusty_v8` se descarga bien a través del proxy.
- Upstream tiene una rama **`search_functions`** que no está en main. Revísala, porque nuestro `search_functions` debería alinearse con ella si encaja.

## Alcance

**Incluye:**

H1. **`docs/cerebro/UPSTREAM.md`**, con evidencia de `archivo:línea` de cada afirmación. Debe responder:
- API de `CodeMode`: `add_callback`, `list_functions`, `get_function_details`, `execute_typescript` y `ToolDisclosure`. Cómo genera pctx el **namespace y el nombre TS** a partir de `CallbackConfig` (namespace o nombre de servidor → camelCase, colisiones). WS3 lo necesita para rellenar `ToolRecord::ts_name`.
- `PctxRegistry::add_callback` y `CallbackFn`. ¿Se puede crear un registry nuevo por ejecución con el mismo `CodeMode`? ¿Qué pasa si falta un id (`Registry missing ids`)?
- Límites del executor:
  - timeout por defecto y cómo cambiarlo;
  - **¿hay límite de memoria/heap de V8?** Si no lo hay, propón un parche (`CEREBRO-PATCH`) por consulta antes de aplicarlo;
  - **permisos de red y FS del sandbox**: queremos **cero red**, porque todo va por callbacks. ¿Lo cumple pctx en modo callbacks?
- Type-check: ¿se ejecuta siempre antes de ejecutar? ¿Cuánto tarda la primera vez y las siguientes? Mide con un ejemplo.
- Formato de `ExecuteTypescriptOutput` (`success`, `stdout`, `stderr`, `output` y `trace`) y cómo llega a `stderr` un `Err(String)` de un callback. Necesitamos que `CerebroErrorPayload::parse_from` lo encuentre: verifícalo con un test.
- `pctx_config`: ¿admite campos extra, o el parser los rechaza?
- `pctx_mcp_server`: qué hace y si nos sirve de referencia para nuestras herramientas.
- La rama upstream `search_functions`: qué propone.
- **Runbook de sincronización con upstream**: fetch, merge, conflictos típicos (`README.md` → conservar el nuestro; `Cargo.lock` → regenerar) y cómo ejecutar los tests upstream.
- Lista de parches `CEREBRO-PATCH` aplicados (vacía al principio).

H2. **ADRs** en `docs/cerebro/adr/` (formato corto: contexto, decisión, alternativas, consecuencias):
- `0002-fork-pctx.md`: estrategia de fork y política de parches.
- `0003-sandbox-deno.md`: sandbox Deno heredado de pctx, límites y por qué se descartó QuickJS.

H3. **Spike de verificación** en `crates/cerebro_spike_pctx/` (crate de pruebas desechable, `publish = false`, solo tests). Tests que prueban:
- un callback registrado se llama desde TS;
- un `Err(CerebroErrorPayload)` llega a `stderr` y se puede parsear;
- un bucle infinito acaba por timeout;
- `fetch("https://example.com")` falla dentro del sandbox;
- si hay límite de memoria, una bomba de memoria no tumba el host.

H4. **CI propia** en `.github/workflows/cerebro-ci.yml`, sin tocar los workflows de upstream: `cargo fmt --check` y `cargo clippy -- -D warnings` sobre `-p cerebro_*`, `cargo test -p cerebro_*` y `pnpm -r build && pnpm -r test` en `packages/`, con caché de `~/.cargo` y `target`. Indica en tu reporte si los workflows de upstream (`release.yml`, `sync-helm.yaml`…) pueden ejecutarse por error en nuestro repo y qué propones (**no los desactives sin consultar**).

**No incluye:** implementar el hub, el store o el servidor. No modifiques nada de pctx sin una consulta aprobada.

## Criterios de aceptación

- `UPSTREAM.md` responde todas las preguntas de H1 con evidencia, o dice explícitamente "no verificado".
- `cargo test -p cerebro_spike_pctx` en verde. Si algún comportamiento no es el deseado, el test lo **documenta** en lugar de ocultarlo.
- El workflow es válido en YAML y sus comandos se han ejecutado en local.

## Consultas previstas (tráelas al coordinador)

- Cualquier parche a pctx, empezando por el límite de memoria y la red.
- Si la rama `search_functions` de upstream hace redundante parte de WS2 o WS4.

## Primer paso

Lee `crates/pctx_code_mode/src/code_mode.rs`, `crates/pctx_registry/src/registry.rs` y `crates/pctx_executor/src/lib.rs`, y envía un `[REPORTE WS1] H1 parcial` en cuanto tengas resueltos el mapeo de nombres TS y el formato de error de callback. WS3 y WS4 dependen de eso.
