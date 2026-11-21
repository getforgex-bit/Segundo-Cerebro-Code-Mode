# Plan v2: mejoras al proyecto **Cerebro** y a su plan

> Este documento revisa el plan v1, que se conserva más abajo como **Anexo A** y sigue vigente en todo lo que la v2 no cambie. Propone mejoras y reordena la Fase 0.
> **Resumen de decisiones v2:**
> - Fork de **pctx** en este repo, con historial, como motor Code Mode (Deno + type-check).
> - Nuestro hub registra los downstream como callbacks, para que la política vea todas las llamadas.
> - Cerebro en SQLite, compartido con el equipo vía **Markdown en git**.
> - Código TS/JS, Rust y Python con tree-sitter + SCIP opcional.
> - Evals en la Fase 1 y claude.ai en la Fase 3.
> - Uso personal y equipo ForgeX.
> **Estado real del repo:** solo hay andamiaje **sin commit**: carpetas vacías y un `Cargo.toml` del workspace. Ese `Cargo.toml` lleva `rusqlite 0.39`, pero `rusqlite_migration 2.7` exige `0.41`, así que hay que corregirlo. No hay código, tests ni docs escritos.

## V2.1 Contexto: por qué revisar el plan

Al empezar la Fase 0 aparecieron tres hechos que el plan v1 no consideraba:
1. **Ya existe un gateway Code Mode open source en Rust.** `pctx` / `pctx_code_mode` 0.7.0 tiene licencia MIT: genera interfaces TS desde los esquemas MCP, ejecuta TS en un **Deno sandbox** (`deno_core` 0.412), enruta las llamadas a servidores MCP o a callbacks de Rust, expone `list_functions` y `get_function_details`, y ofrece modos de exposición (`Catalog`, `Filesystem`, `Sidecar`) y un runtime de type-check. Cubre aproximadamente los spikes B y A-cliente y el typegen del plan v1.
   - Riesgos: lo mantiene sobre todo una empresa (Sideko), sus partes cloud son cerradas, usa `rmcp 1.8`, anterior a la spec 2026-07-28, y V8 aumenta el binario y el tiempo de compilación.
2. **Ya existen cerebros MCP.** **Basic Memory** es local-first: Markdown es la fuente, SQLite con FTS5 y sqlite-vec hace de índice, modela entidades, observaciones y relaciones con `memory://`, y está en Python. **Serena** ofrece símbolos de código vía LSP. Ninguno cubre lo que elegiste: SQLite como fuente **y** un grafo unificado memoria ↔ código ↔ herramientas ↔ skills. Ahí está el diferencial que hay que construir.
3. **La spec MCP 2026-07-28 trae MRTR (SEP-2322).** Las peticiones del servidor al cliente, como la elicitation, ahora van como `InputRequiredResult`: el cliente reintenta la llamada con `input_responses` y `request_state`, y rmcp 3.11 ya modela esos campos. Encaja de forma nativa con el flujo de aprobaciones del v1, que se había diseñado con tokens propios.

**Resultado buscado de la v2:** dejar de invertir donde ya hay soluciones, **validar el valor con datos antes de escribir mucho Rust**, concentrar el esfuerzo en el diferencial y reducir el riesgo de seguridad.

## V2.2 Diagnóstico del plan v1

**Lo que se mantiene:** el principio de *progressive disclosure*, las 5 meta-herramientas, SQLite con export a Markdown, los 4 clientes más claude.ai, la política `allow/confirm/deny`, el arranque perezoso de servidores, `install --replace` y el roadmap por fases.

| # | Hueco del v1 | Impacto |
|---|---|---|
| 1 | No hay un análisis de construir frente a reutilizar | Semanas de trabajo duplicando pctx |
| 2 | El valor no se valida antes de construir: el benchmark llega en la Fase 1 | Riesgo de construir algo que no ahorra o que baja la tasa de éxito con GPT o Gemini, los modelos de Codex y Antigravity |
| 3 | El cerebro y el gateway están acoplados en el diseño | Quien solo quiere memoria no puede adoptarlo, y los tests se complican |
| 4 | Las aprobaciones usan tokens propios en lugar de MRTR | Peor UX en los clientes 2026-07-28 |
| 5 | La política es por herramienta y no por **flujo de datos** | No protege del "trifecta letal": datos privados + contenido no confiable + canal de salida |
| 6 | Los resultados grandes solo se **truncan** | Se pierde información y la IA reintenta a ciegas |
| 7 | El grafo de código solo es sintáctico (tree-sitter) | Las referencias cruzadas son imprecisas, cuando el usuario pidió "interconectar código" |
| 8 | La búsqueda es solo léxica, con descripciones en inglés y consultas que pueden venir en español | Huecos de recall entre idiomas |
| 9 | Faltan nombre, licencia y modelo de uso (personal, equipo o producto) | Bloquea la distribución y el diseño de la sincronización |
| 10 | No hay modelo de amenazas escrito | Las decisiones de seguridad quedan implícitas |

## V2.3 Mejoras propuestas

| # | Mejora | Qué cambia | Beneficio medible |
|---|---|---|---|
| M1 | **Construir frente a reutilizar, con un trait `ExecutionEngine`** | Spike comparativo de 2–3 días: `pctx_code_mode` (Deno) contra un motor propio (rquickjs). Se miden arranque en frío, RAM, tamaño del binario, compatibilidad TS, límites (timeout, OOM, sin red ni FS) y facilidad para meter la política en el puente. Gana el mejor y el trait permite cambiar después. | Si gana pctx, el gateway MVP baja de unas 3–4 semanas a unas 2 (estimación) |
| M2 | **Primero la evidencia: harness de evals en la Fase 0** | `packages/evals` y un fixture **mega-server** con 300+ herramientas realistas (sin credenciales). Línea base con MCP directo **antes** de construir. Se mide con las 3 familias de modelos de los clientes objetivo (Claude, GPT/Codex y Gemini/Antigravity). | Criterio go/no-go objetivo; las métricas del v1 §2 pasan a ser verificables desde el día 1 |
| M3 | **Cerebro desacoplado como servidor MCP interno** | `brain` es un servidor MCP más dentro del mismo binario, con transporte en proceso y sin IPC. Se usa solo (`cerebro brain serve`) o compuesto desde código (`tools.brain.*`). | Adopción por partes, tests aislados, el mismo código en los dos modos |
| M4 | **Aprobaciones nativas con MRTR** | En clientes 2026-07-28 se usa `InputRequiredResult`; en los antiguos, el token de aprobación del v1 queda como fallback. | Confirmación en línea, sin un turno extra del modelo |
| M5 | **Política de flujo ("taint" por ejecución)** | Las herramientas se etiquetan como `private-data`, `untrusted-input` o `external-write`, por anotaciones y config. Si en una ejecución se combinan las tres, se exige confirmación. Solo Cerebro puede hacerlo, porque todas las llamadas pasan por su puente. | Mitiga la exfiltración por inyección de prompts; es un diferencial frente a los clientes nativos |
| M6 | **Resultados como *handles*** | Lo que supera el presupuesto se guarda en `workspace` y se devuelve `{handle, bytes, forma/esquema, muestra}`. La IA pide cortes con `workspace.slice(handle, …)`. Helpers en el runtime: `pick`, `sample`, `table` y `count`. | Menos reintentos y nada de pérdida silenciosa de datos |
| M7 | **Grafo de código en dos niveles** | tree-sitter para símbolos e imports, más un **importador SCIP opcional** (`scip-typescript`, `rust-analyzer scip`, `scip-python`) para referencias precisas (`references` e `implements`). | Navegar memoria ↔ código ↔ referencias con precisión, sin LSP vivo |
| M8 | **Búsqueda multilingüe barata** | Expansión de consulta con un diccionario es↔en de verbos y objetos comunes (crear/create, borrar/delete, incidencia/issue…) en la Fase 1. Los embeddings locales se adelantan solo si el eval muestra un hueco. | Recall@5 ≥ 90 % con consultas en español |
| M9 | **Interoperabilidad de skills** | Exportar skills como **Agent Skills** (`SKILL.md`), que usan Claude Code y Codex, y evaluar servirlas como skills MCP (SEP-2640; rmcp 3.11 trae `skills-fs`). Hay que verificar el estado del SEP antes de comprometerse. | Las skills creadas por la IA se reutilizan fuera de Cerebro |
| M10 | **Modelo de amenazas** | `docs/threat-model.md` con activos, actores, superficies (sandbox, puente, modo remoto, skills, índices) y controles. | Decisiones de seguridad explícitas y auditables |
| M11 | **DX y distribución** | Wrapper `npx <nombre>` con binarios precompilados (`cargo-dist`), porque los usuarios de los 4 clientes ya tienen Node. `init` autodetecta los clientes instalados; `doctor` verifica la config, el ahorro real y que no queden servidores duplicados. | Instalación en un comando |
| M12 | **Gestión del proyecto** | Hitos e issues en GitHub por fase, *Definition of Done* por fase, un ADR por decisión, y decidir nombre y licencia. **"Cerebro" choca con un proyecto conocido**: lmenezes/cerebro, la consola de Elasticsearch, y seguramente también en crates.io y npm. | Menos fricción para publicar y colaborar |

**Estado de las mejoras tras tus respuestas:**
- **Adoptadas:** M1 (como *fork* de pctx), M3–M7, M9–M11, M12 (sin presión por el nombre, al ser de uso interno).
- **Pospuestas:** M2, a la Fase 1; M8, a la Fase 1.
- Detalle en V2.4–V2.8.

## V2.4 Decisiones del usuario (ronda v2)

| Tema | Decisión | Consecuencia en el plan |
|---|---|---|
| Motor Code Mode | **Fork de pctx** | M1 pasa a "fork": el sandbox QuickJS y oxc del v1 **se descartan**. El motor es Deno (`deno_core`) con el type-check de pctx. |
| Estructura del fork | **En este repo, con historial**, con remoto `upstream` | Los crates `cerebro_*` conviven con los `pctx*`, con parches mínimos marcados. Los parches genéricos se proponen upstream. |
| Validación | **Construir y medir después** | M2 pasa a la Fase 1 (evals tal como en el v1). En la Fase 0 solo se comprueba el tamaño de `tools/list` en el E2E. |
| Usuarios | **Uso personal + equipo ForgeX** (interno, no OSS público) | No hace falta un nombre único público; "Cerebro" vale como nombre interno. Hay que conservar la licencia MIT de pctx (aviso de copyright). Nuestro código queda en uso interno; la licencia se decide si algún día se publica. |
| claude.ai | **No prioritario** | El modo remoto pasa a la Fase 3, sin cambios en el MVP. |
| Cerebro en equipo | **Markdown en git** | Se añade un diseño de sincronización (V2.6). `brain.db` queda en `.gitignore`. |
| Lenguajes de código | **TS/JS, Rust, Python** | Gramáticas `tree-sitter-{typescript,javascript,rust,python}` |
| Precisión del código | **tree-sitter + SCIP opcional** | M7 se adopta (`scip-typescript`, `rust-analyzer scip`, `scip-python`) |

## V2.5 Arquitectura resultante (sustituye §3, §4, §5 y §10 del Anexo A donde haya conflicto)

**Idea clave:** se usa pctx como **motor** (genera TS a partir de los esquemas, sandbox Deno, type-check y enrutado de *callbacks*). **Todo lo que llega a un servidor downstream pasa por nuestro hub**: cada herramienta upstream se registra en pctx como **callback** (`PctxRegistry::add_callback`) y no con `add_mcp`. Así nuestra política, auditoría, *taint*, aprobaciones y arranque perezoso ven **todas** las llamadas, sin parchear el registry de pctx. Además se evita el `rmcp 1.8` interno de pctx para hablar con los downstream, porque nuestro hub usa `rmcp 3.11` (spec 2026-07-28 y MRTR).

```
repo (fork de portofcontext/pctx con historial; remoto `upstream`)
├── crates/pctx*/            upstream: codegen, executor (Deno), type-check, registry, config, mcp_server…
│                            → parches mínimos, marcados con `// CEREBRO-PATCH:` y listados en docs/UPSTREAM.md
├── crates/cerebro_store/    SQLite (rusqlite 0.41 + rusqlite_migration): migraciones hub/brain, pragmas, ATTACH
├── crates/cerebro_search/   FTS5 + trigram + RRF (+ sinónimos es↔en, fase 1) sobre catálogo y cerebro
├── crates/cerebro_hub/      cliente MCP downstream (rmcp 3.11), ciclo de vida perezoso, upstream→callback
├── crates/cerebro_policy/   allow/confirm/deny, taint por ejecución, aprobaciones (MRTR + token), auditoría
├── crates/cerebro_brain/    memorias, grafo, extractores (md, tree-sitter TS/JS/Rust/Python, config), SCIP, sync MD
├── crates/cerebro/          binario: CLI + servidor MCP upstream (rmcp 3.11: stdio, después HTTP) + install/import-config
├── packages/                TS: fixtures (echo, destructive, injection, large-output), e2e (SDK oficial), evals (fase 1)
└── docs/                    PLAN, adr/, UPSTREAM.md (runbook del fork), threat-model.md, clients.md, meta-tools.md, sandbox-api.d.ts
```

**Superficie para la IA (6 herramientas), con objetivo ≤ 2k tokens:**
- `search_functions`: nuestra búsqueda FTS sobre el catálogo; devuelve una línea por función.
- `get_function_details`: la de pctx; devuelve la firma TS.
- `execute_typescript`: la de pctx, envuelta para añadir límites, *handles* (M6) y el resumen de llamadas.
- `cerebro_recall`, `cerebro_remember` y `cerebro_primer`: las del cerebro.
- `list_functions` de pctx queda **desactivada por defecto**, porque lista todo y es cara con muchas herramientas; se puede activar en la config.

**Dentro del sandbox** se siguen las convenciones de pctx: cada servidor es un *namespace* TS (`await github.createIssue({...})`). Nuestros namespaces son `Brain`, `Workspace` y `Skills`, también registrados como callbacks. `docs/sandbox-api.d.ts` se rehace con estas convenciones una vez verificadas en el código del fork.

**Config:** se adopta el formato `pctx.json` de upstream, con secretos `${env:}`, `${keychain:}` y `${command:}`. Lo nuestro va en `.cerebro/config.json` (cerebro, política, límites) si `pctx_config` no admite campos extra; esto se verifica en 0b. Así se evita divergir del parser upstream. El TOML del v1 **se descarta**.

## V2.6 Cerebro de equipo: sincronización vía Markdown en git

- **Visibilidad por memoria:** `team` por defecto, o `personal`, que nunca se exporta. Columnas nuevas en `memories`: `visibility`, `author`, `sync_hash` y `synced_at`.
- **Formato:** `.cerebro/brain/<kind>/<slug>--<uid8>.md`, un archivo por memoria para que git combine por archivo. Lleva frontmatter YAML (uid, kind, status, importance, tags, links, author, created/updated) y el cuerpo en Markdown.
- **`cerebro brain sync`** es idempotente:
  1. Importa los archivos cuyo hash difiere del `sync_hash`.
  2. Exporta lo que cambió en la BD desde la última sincronización.
  3. Si ambos lados cambiaron, escribe `<archivo>.conflict.md` y marca `status=conflict`. Nada se sobrescribe en silencio.
  4. En `serve` hay un watcher sobre `.cerebro/brain/` y un hook opcional `post-merge` de git.
- **En git:** `.cerebro/brain/**` se versiona. `.cerebro/brain.db*`, `.cerebro/cache/` y `.cerebro/workspace/` van en `.gitignore`.
- **Qué no se exporta:** el índice de código ni los chunks, porque se pueden reconstruir. Sí se exportan los **enlaces** memoria→código en el frontmatter.

## V2.7 Fase 0 revisada (sustituye §14 del Anexo A)

| Paso | Entregable | Criterio |
|---|---|---|
| 0a | **Importar el fork:** borrar el andamiaje sin commit (`Cargo.toml` y carpetas vacías), `git remote add upstream https://github.com/portofcontext/pctx`, `git fetch upstream`, `git merge --allow-unrelated-histories upstream/main`. El conflicto de `README.md` se resuelve con nuestro README, que atribuye a pctx (MIT). | `cargo build` y los tests upstream en verde en este entorno |
| 0b | **Leer el fork** y documentar los puntos de extensión en `docs/UPSTREAM.md`: API de `CodeMode` y `PctxRegistry`, límites del executor (timeout 10 s y ¿memoria?), permisos de red de Deno (objetivo: **sin red**, porque todo va por callbacks), si `pctx_config` acepta campos extra, la versión de `rmcp` en `main` y cómo funciona `pctx_mcp_server`. Incluye el runbook de sincronización con upstream. | ADR-0002 (estrategia de fork y política de parches) |
| 0c | `cerebro_store`: migraciones v1 (Anexo A §8) ampliadas con `visibility`, `author`, `sync_hash`, `taint_labels`, `result_handles` y `sync_state`, más tests (FTS en insert, update, delete y cascada; `bm25()`) | `cargo test -p cerebro_store` |
| 0d | `cerebro_hub` (rmcp 3.11): conectar por stdio con los fixtures, `tools/list` → catálogo en SQLite, y **registrar cada herramienta como callback de pctx** | Test: TS en el sandbox llama a `echo.echo()` y pasa por el hub |
| 0e | `cerebro_policy` mínimo: `allow/confirm/deny` por anotación más el token de aprobación. Se intenta MRTR si el servidor rmcp 3.11 lo soporta. | Test: `destructive.*` pide aprobación y con el token se ejecuta |
| 0f | Binario `cerebro serve --stdio` con las 6 herramientas: `search_functions` real; `recall`, `remember` y `primer` como stubs | E2E (`packages/e2e`, SDK TS oficial): `tools/list` devuelve 6, se registra el tamaño y `execute_typescript` funciona de punta a punta |
| 0g | Docs: PLAN v2, ADRs 0001–0006, `UPSTREAM.md`, `threat-model.md`, `clients.md` (Claude Code, Claude Desktop, Codex, Antigravity), `meta-tools.md`, `sandbox-api.d.ts` | `tsc --noEmit` sobre el `.d.ts` |
| 0h | CI: tests upstream y los nuestros, con caché de `rusty_v8`; `fmt`, `clippy` y `pnpm -r test` | Workflow en verde (al menos en local) |
| 0i | Commits descriptivos y `git push -u origin ccr-f6dcb0c5-1kktmp`, sin PR | — |

**ADRs v2:**
- 0001: núcleo Rust con TS como lenguaje invitado (sigue vigente).
- 0002: fork de pctx en el repo y política de parches.
- 0003: sandbox Deno heredado de pctx; QuickJS descartado.
- 0004: SQLite como fuente local más Markdown en git para el equipo.
- 0005: downstream a través de nuestro hub como callbacks, con la política primero.
- 0006: grafo de código con tree-sitter y SCIP opcional.

## V2.8 Roadmap ajustado

- **Fase 0, fork e integración (~1–1,5 semanas):** V2.7.
- **Fase 1, MVP gateway (~2–3 semanas):** catálogo completo y sincronización perezosa, búsqueda con sinónimos (M8), política completa con MRTR (M4), *handles* (M6), auditoría y `stats`, `import-config` e `install --client [--replace]` para Claude Code, Claude Desktop, Codex y Antigravity, `doctor`, wrapper npm, y **evals** (M2: línea base contra Cerebro con las 3 familias de modelos).
- **Fase 2, segundo cerebro (~4 semanas):** memorias, `recall`, `remember` y `primer`, sincronización Markdown en git (V2.6), extractores md y config, tree-sitter TS/JS, Rust y Python, grafo con enlaces automáticos, importador SCIP, watcher, taint de flujo (M5) y skills.
- **Fase 3:** remoto Streamable HTTP + OAuth para claude.ai, skills interoperables (M9), embeddings si el eval lo justifica, y extractores CSV, ipynb, PDF, DOCX y git.
- **Fase 4:** dashboard, endurecimiento y empaquetado (sin cambios).

## V2.9 Riesgos nuevos por el fork

| Riesgo | Mitigación |
|---|---|
| **`rusty_v8` descarga un binario precompilado** desde los releases de GitHub al compilar `deno_core`, y el proxy de este entorno o de la CI podría bloquearlo | Verificarlo en 0a. Si falla: `RUSTY_V8_MIRROR` o `RUSTY_V8_ARCHIVE`, caché en CI. En el peor caso, documentar el bloqueo y pedir que se permita el host. |
| Divergencia del fork y conflictos al sincronizar con upstream | Código nuestro en crates aparte, parches mínimos y marcados, PRs upstream de los genéricos, y sincronización mensual con el runbook en `UPSTREAM.md` |
| Dos versiones de rmcp (1.8 en pctx, 3.11 nuestro) | Aceptable porque solo intercambian JSON. Proponer upstream la subida a rmcp 3.x. |
| Binario grande y compilación lenta por V8 | `sccache` y caché en CI; binarios precompilados con `cargo-dist` en la Fase 1 |
| Un solo mantenedor upstream (Port of Context) | Con un fork MIT no dependemos de su continuidad |

## V2.10 Verificación de la v2

- 0a: `cargo build --workspace` y `cargo test` de los crates upstream en verde tras el merge (o el bloqueo de `rusty_v8` documentado con su solución).
- 0c–0f: `cargo test --workspace`, más un E2E con el SDK TS: 6 herramientas, `execute_typescript` → callback → hub → fixture `echo`, y `destructive` pide aprobación.
- `cargo clippy --workspace -- -D warnings` en nuestros crates (el lint de upstream se respeta tal cual), y `tsc --noEmit` sobre `docs/sandbox-api.d.ts`.
- Manual, para ti: `claude mcp add cerebro -- <ruta>/cerebro serve --stdio`, luego `/mcp` para ver las 6 herramientas, y probar una búsqueda y una ejecución.

## V2.10b Decisiones menores con valor por defecto (cámbialas cuando quieras)

- **PRs upstream a pctx:** solo cuando tú lo apruebes caso por caso, porque son acciones públicas en nombre de ForgeX. Mientras tanto, los parches viven en el fork.
- **Dedicación:** las estimaciones suponen 1 desarrollador con IA. Con más gente, las fases 1 y 2 pueden ir en paralelo (gateway y cerebro están desacoplados por M3).
- **Embeddings:** se deciden en la Fase 3 con datos del eval. Por defecto, solo FTS + trigram + sinónimos.
- **Licencia de nuestro código:** uso interno ForgeX. Los archivos de pctx conservan su MIT, y `LICENSE` y `NOTICE` atribuyen el upstream.

## V2.11 Fuentes nuevas

- pctx / pctx_code_mode (MIT): https://github.com/portofcontext/pctx · docs: https://github.com/portofcontext/pctx/blob/main/docs/code-mode.md · https://github.com/portofcontext/pctx/blob/main/docs/config.md · https://docs.rs/crate/pctx_code_mode/0.5.0
- Basic Memory: https://github.com/basicmachines-co/basic-memory
- MRTR, spec 2026-07-28: https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/mrtr · SEP-2322: https://modelcontextprotocol.io/seps/2322-MRTR.md
- Otras implementaciones Code Mode: https://github.com/modus-data/mcp_codemode · https://github.com/elusznik/mcp-server-code-execution-mode

---

# Anexo A: Plan v1, **Cerebro**, un segundo cerebro por proyecto con un gateway MCP al estilo Code Mode

> **Partes del v1 que la v2 reemplaza:**
> - Sandbox QuickJS + oxc (§3 decisión 2, §6 pasos 1–2): ahora Deno vía pctx.
> - Runtime `Proxy` propio y `tools.*` (§3 decisión 3, §5): ahora namespaces de pctx.
> - Config TOML (§3 decisión 5): ahora `pctx.json` + `.cerebro/config.json`.
> - 5 meta-herramientas (§4): ahora 6 (V2.5).
> - Estructura del repo (§10), roadmap (§11) y Fase 0 (§14): ver V2.5, V2.7 y V2.8.
>
> El resto sigue vigente: métricas, esquema SQLite (ampliado en V2.7-0c), seguridad, extractores, riesgos y costes.

> Stack: **Rust** (núcleo), **TypeScript** (el lenguaje en que la IA programa, más los SDK) y **SQLite** (memoria, grafo, catálogo y auditoría).
> Repo: `getforgex-bit/Segundo-Cerebro-Code-Mode` (hoy solo tiene `README.md`), rama `ccr-f6dcb0c5-1kktmp`.
> Decisiones del usuario: alcance **Docs + Fase 0**; memoria en **SQLite + export Markdown**, con **indexado e interconexión de varios tipos de archivo, incluido código**; clientes prioritarios: **Claude Code, Claude Desktop/claude.ai, Codex Desktop y Antigravity 2.0**.

---

## 1. Contexto

**Problema:**
1. **Contexto inflado por MCP.** Los clientes cargan todas las definiciones de herramientas en la ventana de contexto, y cada resultado intermedio pasa por el modelo. Con 10–20 servidores son decenas o cientos de miles de tokens antes de empezar. Eso cuesta dinero y degrada la calidad.
2. **Sin memoria de proyecto.** Cada sesión empieza de cero. Las decisiones, convenciones y tareas, y la relación entre ellas y el código, se pierden o acaban en archivos tipo `CLAUDE.md` que se cargan enteros.

**Inspiración verificada:**
- *Anthropic, "Code execution with MCP"*: los servidores se presentan como una API de código. Hay `search_tools` con niveles de detalle, los datos se filtran en el sandbox, el estado vive en `./workspace/`, hay `./skills/` reutilizables y tokenización de PII. En su ejemplo, ~150k → ~2k tokens (−98,7 %).
- *Cloudflare, Code Mode*: dos herramientas, `search()` y `execute()`, que reciben código. Un isolate V8 sin FS, env ni red, y credenciales fuera del sandbox. Unos 1.000 tokens fijos para más de 2.500 endpoints. Su límite reconocido: **una sola API**.
- *Spec MCP 2026-07-28*: protocolo stateless (`server/discover`, `resultType`). Roots, Sampling, Logging y HTTP+SSE quedan deprecados, así que hay que usar Streamable HTTP.

**Resultado buscado:** un binario local que los cuatro clientes conectan como **un único servidor MCP**. Agrega N servidores MCP *sin* exponer sus esquemas. La IA **busca** la herramienta, **escribe TypeScript** que la usa en un sandbox y recibe **solo el resultado final**. Encima hay un **grafo de conocimiento del proyecto** (memorias ↔ archivos ↔ símbolos de código ↔ skills ↔ herramientas) que se consulta igual, por búsqueda y sin cargarlo entero.

**Principio unificador:** *progressive disclosure*. En el contexto vive un **índice**, no el contenido. El catálogo de herramientas y el cerebro del proyecto son corpus indexados en SQLite que comparten el motor de búsqueda y el sandbox.

---

## 2. Métricas de éxito (objetivos que hay que medir, no garantías)

| Métrica | Objetivo | Medición |
|---|---|---|
| Tokens fijos que añade Cerebro | ≤ 2.000, con independencia del número de servidores | `messages.count_tokens` con y sin Cerebro |
| Ahorro en tareas multi-paso | ≥ 80 % frente a MCP directo (hipótesis) | Benchmark de 10–20 tareas en `packages/evals` |
| Recall@5 de `search_tools` | ≥ 90 % | Golden set de ~100 consultas |
| Recall@5 del cerebro (memorias + código) | ≥ 85 % | Golden set de preguntas del proyecto |
| Éxito de tareas | No más de 5 pp por debajo de MCP directo | Mismo benchmark, varios modelos |
| Overhead de `execute` | p95 < 50 ms | Trazas `tracing` |
| Búsqueda con 5.000 herramientas / 50.000 chunks | p95 < 30 ms / < 60 ms | `criterion` |

`cerebro stats` muestra el ahorro por proyecto: bytes intermedios que **no** pasaron por el modelo frente a lo devuelto.

---

## 3. Arquitectura

```mermaid
flowchart LR
  subgraph CLIENTS["Clientes IA"]
    CC[Claude Code] & CD[Claude Desktop] & CX[Codex Desktop] & AG[Antigravity 2.0]
    CAI[claude.ai web]
  end
  CC & CD & CX & AG -- "MCP stdio (local)" --> SRV
  CAI -- "MCP Streamable HTTP<br/>(remoto: túnel o VPS + auth)" --> SRV

  subgraph CEREBRO["cerebro (binario Rust)"]
    SRV[Servidor MCP upstream (rmcp)<br/>5 meta-herramientas ~1,5k tokens]
    SRCH[Búsqueda híbrida<br/>FTS5 BM25 + trigram + RRF<br/>(+ vectores, fase 3)]
    SBX[Sandbox<br/>oxc quita tipos TS → QuickJS<br/>+ @cerebro/runtime]
    POL[Política + auditoría]
    HUB[Hub MCP downstream (rmcp)<br/>arranque perezoso, salud, caché]
    BRAIN[Cerebro: memorias + grafo<br/>extractores: md, código (tree-sitter),<br/>config, datos, notebooks, pdf…]
    SRV --> SRCH & SBX & BRAIN
    SBX -- "puente host (único canal)" --> POL --> HUB
    SBX --> BRAIN
  end
  HUB --> S1[(github)] & S2[(slack)] & S3[(postgres)] & S4[(...)]
  CEREBRO --- DB1[("hub.db (global)")]
  CEREBRO --- DB2[("&lt;proyecto&gt;/.cerebro/brain.db")]
```

### Responsabilidades

| Capa | Responsabilidad | Justificación |
|---|---|---|
| **Rust** | Servidor MCP upstream (stdio y Streamable HTTP), hub downstream, sandbox, política, SQLite, búsqueda, extractores e indexador incremental, CLI, todo en **un solo binario** | Seguridad de memoria donde se ejecuta código no confiable, control de límites, distribución sencilla y rendimiento al indexar. |
| **TypeScript** | (1) El **lenguaje que escribe la IA**. (2) `@cerebro/runtime`, el prelude del sandbox (proxies `tools`, `brain`, `skills`, `workspace`), compilado y embebido con `include_str!`. (3) Los `.d.ts` generados que lee la IA. (4) Las skills. (5) Los fixtures MCP, E2E y evals con los SDK oficiales. (6) `@cerebro/client` para agentes sin MCP (fase 4, no priorizado). | Los LLM escriben TS muy bien, los tipos son documentación compacta y el ecosistema MCP de referencia está en TS. |
| **SQLite** | `hub.db` global (catálogo, firmas, estadísticas) y `brain.db` por proyecto (memorias, documentos, símbolos, chunks, grafo de enlaces, skills, sesiones, ejecuciones, aprobaciones, workspace), unidos con `ATTACH` | Sin infraestructura y portable. FTS5 nativo, WAL y ACID. |

**Decisiones clave** (se documentan como ADR):
1. **Un binario Rust con TS como lenguaje invitado**, en lugar de un orquestador Node con addon nativo.
2. **Sandbox QuickJS (`rquickjs`)** detrás de un trait `SandboxEngine`, en un proceso worker separado desde la fase 1. Si el spike B falla, el plan B es `deno_core`. Sin type-check: solo se quitan los tipos con `oxc`, y la validación de argumentos se hace en el host con `jsonschema`, con errores "didácticos".
3. **Runtime genérico con `Proxy`.** `tools.github.createIssue()` se resuelve en tiempo de ejecución, y los `.d.ts` son solo documentación.
4. **Servidores downstream perezosos**, con el catálogo cacheado por hash de esquema y refrescado por TTL, `cerebro sync` o `tools/list_changed`.
5. **Config en TOML y estado en SQLite.** `~/.config/cerebro/config.toml` y `<proyecto>/.cerebro/config.toml`. Los secretos solo como referencias (`env:` o `keyring:`).
6. **Integración sin fricción con los 4 clientes.** `cerebro import-config` lee los servidores que ya tienes, y `cerebro install --client <x> [--replace]` registra Cerebro en el cliente. Con `--replace` desactiva los servidores originales en ese cliente, con backup y diff previo. **Sin este paso no hay ahorro**, porque el cliente seguiría cargando los esquemas originales.

| Cliente | Transporte | Config que se lee y escribe (verificar al implementar) |
|---|---|---|
| Claude Code | stdio | `.mcp.json` (proyecto), `~/.claude.json` y `claude mcp add` |
| Claude Desktop | stdio | `claude_desktop_config.json` (fase 4: paquete `.mcpb`) |
| Codex Desktop (comparte config con la CLI y el IDE) | stdio / Streamable HTTP | `~/.codex/config.toml` y `.codex/config.toml`, tablas `[mcp_servers.<n>]` |
| Antigravity 2.0 | stdio / HTTP (`serverUrl`) | `~/.gemini/config/mcp_config.json` y `.agents/mcp_config.json` (`mcpServers`, `disabled`, `disabledTools`) |
| claude.ai web | **solo remoto** | Conector personalizado: URL pública accesible desde Anthropic, cabecera bearer (fase 1) u OAuth 2.1 (fase 3) |

---

## 4. Superficie expuesta a la IA (el único contexto fijo)

Las descripciones van en inglés para que sean consistentes entre modelos (configurables), con un objetivo de ≤ 1.500 tokens en total, más unas `instructions` MCP breves.

| Herramienta | Entrada | Devuelve |
|---|---|---|
| `cerebro_search_tools` | `query`, `detail: names\|summary\|full` (por defecto summary), `server?`, `limit?` (8, máx. 25) | Texto compacto, una línea por herramienta (`github.createIssue — Create an issue [write]`). Con `full`, la firma TS con JSDoc. Incluye las skills. |
| `cerebro_execute` | `code` (el **cuerpo** de una función async: `await` y `return`), `timeout_ms?`, `approvals?` | `{ result, logs, calls:[{tool,ms,ok}], truncated, error? }`. Solo vuelve lo que el código retorna, recortado a un presupuesto. |
| `cerebro_recall` | `query`, `scope?: memories\|docs\|code\|all`, `kinds?`, `limit?` | Memorias, fragmentos y símbolos de código relevantes con su referencia (`memory:…`, `file:…#L10-40`, `symbol:…`) y sus vecinos directos del grafo |
| `cerebro_remember` | `kind`, `title`, `body`, `tags?`, `importance?`, `supersedes?`, `links?` | La memoria creada. Los `[[wiki-links]]` del cuerpo se enlazan automáticamente. |
| `cerebro_primer` | `focus?` | Resumen del proyecto en ≤ 1.500 tokens: decisiones y convenciones activas, tareas abiertas, última sesión, mapa de módulos y servidores conectados |

Opcional, desactivada por defecto: `cerebro_call({tool, input})`, una llamada directa sin código. Fase 3: espejo en MCP resources (`cerebro://types/…`, `cerebro://memory/…`).

## 5. API dentro del sandbox (`docs/sandbox-api.d.ts`)

```ts
declare const tools: Tools;            // tools.<servidor>.<herramienta>(input); los .d.ts generados amplían `interface Tools {}`
declare const catalog: { search(q: string, o?: {limit?: number}): Promise<ToolInfo[]>; describe(name: string): Promise<string> };
declare const brain: {
  search(q: string, o?: { scope?: "memories"|"docs"|"code"|"all"; kinds?: MemoryKind[]; limit?: number }): Promise<BrainHit[]>;
  get(ref: NodeRef): Promise<BrainNode | null>;
  remember(m: NewMemory): Promise<Memory>; update(uid: string, p: MemoryPatch): Promise<Memory>; supersede(uid: string, m: NewMemory): Promise<Memory>;
  link(from: NodeRef, to: NodeRef, relation: Relation): Promise<void>;
  neighbors(ref: NodeRef, o?: { relations?: Relation[]; depth?: 1|2; limit?: number }): Promise<Edge[]>;
  read(path: string, range?: { startLine: number; endLine: number }): Promise<string>;
  symbols(q: string, o?: { kind?: SymbolKind; path?: string; limit?: number }): Promise<SymbolInfo[]>;
};
declare const skills: { list(): Promise<SkillInfo[]>; save(d: SkillDef): Promise<SkillInfo>; run<T>(name: string, args?: object): Promise<T> };
declare const workspace: { get<T>(k: string): Promise<T|undefined>; set(k: string, v: unknown, o?: {ttlSeconds?: number; scope?: "session"|"project"}): Promise<void>; delete(k: string): Promise<void>; keys(prefix?: string): Promise<string[]> };
declare function log(...a: unknown[]): void;
declare const limits: { timeoutMs: number; maxToolCalls: number; maxResultBytes: number; remainingToolCalls(): number };
declare class CerebroError extends Error { code: "TOOL_NOT_FOUND"|"INVALID_INPUT"|"NEEDS_APPROVAL"|"DENIED"|"TOOL_ERROR"|"TIMEOUT"|"LIMIT_EXCEEDED"|"SERVER_UNAVAILABLE"; approvalToken?: string; suggestions?: string[] }
// NodeRef = `memory:<uid>` | `file:<ruta>` | `symbol:<ruta>#<nombre.cualificado>` | `skill:<n>` | `tool:<srv.tool>` | `commit:<sha>`
// No existen fetch, import, require, process ni fs.
```
- `ToolResult` ofrece `text`, `data` (`structuredContent`), `content[]`, `isError` y `json<T>()`. Si hay `outputSchema`, el retorno generado es tipado.
- Una herramienta que devuelve `isError` lanza `TOOL_ERROR`. `tools.$call(name, input, {throwOnError:false})` da acceso crudo. Los nombres snake_case se mapean a camelCase y el mapeo se guarda en SQLite. `TOOL_NOT_FOUND` incluye sugerencias.

## 6. Flujo de `cerebro_execute`

1. Quitar los tipos con oxc y comprobar la sintaxis (los errores salen con línea y columna).
2. Tomar un worker del pool y cargar el prelude con los límites: 64 MB, 30 s, 50 llamadas y 20 KB de resultado (configurables).
3. Para cada `tools.x.y()`, el **puente host**: resuelve en el catálogo → aplica la **política** → valida con JSON Schema → arranca el servidor si hace falta → llama vía rmcp → registra en `tool_calls` (con argumentos redactados) → devuelve el resultado **al sandbox, no al modelo**.
4. Al terminar: serializa, recorta, tokeniza la PII si está activado (fase 3) y registra en `executions` (`raw_bytes_processed` frente a `result_bytes`).

**Confirmaciones** (herramientas `destructiveHint` o marcadas `confirm`): por *elicitation* MCP si el cliente la soporta. Si no, la llamada devuelve `NEEDS_APPROVAL` con un `approvalToken` de un solo uso ligado al hash de los argumentos. La IA pregunta al usuario y vuelve a ejecutar con `approvals:[token]`. Funciona con cualquier cliente.
**Ojo con los clientes:** si el usuario permite `cerebro_execute` sin preguntar (en Antigravity, `mcp(cerebro/*)`; en Claude Code o Codex, mediante allowlist), la responsabilidad de confirmar pasa a la política de Cerebro. Esto queda documentado en `docs/clients.md`.

## 7. Cerebro del proyecto: tipos de archivo e interconexión

**Pipeline de extractores** (trait `Extractor` en Rust, cada uno con `extractor_version` para reindexar):

| Tipo | Extractor | Qué produce | Fase |
|---|---|---|---|
| Markdown / MDX / texto | `pulldown-cmark` | Chunks por encabezado y enlaces `[[…]]` o relativos | 2 |
| Código: TS/JS, Rust, Python, Go (después Java, C#, PHP, SQL…) | `tree-sitter` + gramáticas | **Símbolos** (función, método, clase, struct, trait, interfaz, tipo, constante, test), firma, doc-comment, rango de líneas, padre, y **imports** convertidos en enlaces `imports` | 2 |
| Lenguajes sin gramática | Ventanas de líneas con solapamiento | Chunks | 2 |
| Config: JSON / YAML / TOML | `serde_*` | Rutas de claves de primer y segundo nivel; detecta `package.json`, `Cargo.toml` y similares para dependencias | 2 |
| Datos: CSV | `csv` | Cabecera, tipos inferidos y muestra (no el contenido entero) | 3 |
| Notebooks `.ipynb` | JSON | Chunks por celda (código y markdown) | 3 |
| PDF / DOCX | `pdf-extract` o `lopdf` / zip + xml | Texto por página o sección | 3 |
| Git | `gix` | Nodos `commit:` con enlaces `changed` a archivos y decisiones | 3 |
| Imágenes / binarios | — | Solo metadatos; se omiten por tamaño y `.cerebroignore` | 3 |

- **Indexado incremental:** `ignore` (respeta `.gitignore` y `.cerebroignore`) y blake3 por archivo. Solo se reextrae lo que cambia. En `serve` hay un watcher (`notify`), y también existe `cerebro index [--full]`.
- **Grafo:** la tabla `links` usa refs URI (`memory:`, `file:`, `symbol:`, `skill:`, `tool:`, `commit:`) y relaciones `relates_to, supersedes, depends_on, implements, mentions, imports, defines, tests, documents, blocks, derived_from, changed`, con `origin` (auto, user o agent) y `confidence`. Los enlaces automáticos salen de los imports, de los wiki-links `[[src/lib.rs#execute]]` y de las menciones de rutas o símbolos conocidos en las memorias.
- **Export/import Markdown:** las memorias se exportan con frontmatter YAML (uid, kind, status, tags y enlaces), compatible con Obsidian y comparable en git. El código no se exporta, porque ya está en el repo, solo sus enlaces.

## 8. Modelo de datos SQLite

La fuente única son las migraciones en `crates/cerebro-core/migrations/{hub,brain}/0001_init.sql`, y la documentación las enlaza.

**`hub.db`:**
- `servers`: estado y metadatos. El comando y los secretos no se guardan aquí, viven en el TOML.
- `tools`: `name`, `ts_name`, `description`, `input_schema`, `output_schema`, `annotations`, `ts_signature`, `ts_declaration`, `search_text`, `approx_tokens` (línea base), `schema_hash`, `aliases`, `available`.
- `tools_fts` (porter) y `tools_trgm` (trigram), ambas external content con triggers.
- `tool_stats`.
- Fase 3: `tools_vec` (sqlite-vec, pre-v1) con embeddings locales (`fastembed`), solo si el eval demuestra mejora.

**`brain.db`:**
- `memories`: uid ULID, kind ∈ {decision, convention, fact, note, task, question, glossary, incident, preference, summary}, status, importance, tags JSON, source, session y supersedes.
- `memories_fts` con `unicode61 remove_diacritics 2` **sin porter** (el contenido estará en español) y `prefix='2 3'`, más una tabla trigram.
- `documents` (ruta, kind ∈ {markdown, code, config, data, notebook, pdf, office, text, other}, language, hash, size, mtime, extractor y su versión), `symbols` (con `symbols_fts`) y `chunks` (`symbol_id?`, con `chunks_fts`).
- `links`, `skills` (con `skills_fts` y `trusted`), `sessions`, `executions`, `tool_calls`, `approvals`, `workspace_kv` y la vista `v_token_savings`.
- Pragmas: WAL, `foreign_keys`, `busy_timeout=5000`. Migraciones con `rusqlite_migration`.

## 9. Seguridad

- **Sandbox:** sin FS, red, env ni `import`; solo las funciones del puente. Proceso aislado con rlimits (seccomp, landlock o Wasm en fase 4). Límites de heap, CPU, llamadas, salida y logs.
- **Credenciales:** solo los procesos downstream las ven. Nunca llegan al sandbox, a SQLite ni a los logs. Los argumentos se redactan en la auditoría y se guarda su hash.
- **Política** `allow | confirm | deny` por servidor o herramienta, con valores por defecto sacados de las anotaciones MCP: `readOnlyHint` → allow, `destructiveHint` → confirm, lo desconocido de escritura → confirm.
- **Inyección de prompts:** los resultados son datos y lo destructivo siempre pide confirmación. Las skills nuevas quedan `trusted=0` hasta `cerebro skills approve`. Tokenización de PII opcional.
- **Modo remoto (claude.ai):** expone *todas* tus herramientas a internet, así que usa un perfil `remote` más restrictivo (todo lo que escribe pide confirmación, con allowlist de servidores). Bearer token obligatorio (cabecera configurable en el conector), validación de `Origin`, rate limit y TLS en el túnel (Cloudflare Tunnel, Tailscale Funnel o un VPS). El cerebro y los servidores stdio viven en esa máquina. OAuth 2.1 llega en la fase 3.

## 10. Estructura del repositorio

```
Cargo.toml                       # workspace
crates/
  cerebro-core/                  # config, store (+ migrations/), search, brain, extractors, policy, typegen
  cerebro-hub/                   # cliente MCP downstream (rmcp) + ciclo de vida
  cerebro-sandbox/               # oxc + rquickjs + puente; worker aislado
  cerebro/                       # bin: CLI (clap) + servidor MCP upstream (stdio / Streamable HTTP)
packages/                        # pnpm workspace
  runtime/                       # @cerebro/runtime → JS embebido
  fixtures/                      # servidores MCP de prueba (echo, slow, large-output, destructive, flaky, injection)
  e2e/                           # tests con @modelcontextprotocol/sdk contra `cerebro serve`
  evals/                         # benchmarks (fase 1)
docs/  PLAN.md · adr/ · sandbox-api.d.ts · meta-tools.md · clients.md
.github/workflows/ci.yml
```

Dependencias (las versiones se fijan y se verifican en docs.rs y npm al implementar): `tokio`, `rmcp` (SDK oficial, 3.x), `rusqlite` (bundled), `rusqlite_migration`, `rquickjs`, `oxc`, `jsonschema`, `serde_json`, `blake3`, `ulid`, `ignore`, `notify`, `keyring`, `clap`, `axum`, `tracing`, `insta`, `criterion`; en fase 2, `tree-sitter` y `pulldown-cmark`. En TS: `typescript`, `esbuild`, `vitest`, `@modelcontextprotocol/sdk`, y `@anthropic-ai/sdk` para los evals con `count_tokens`.

## 11. Roadmap (estimaciones para 1 desarrollador con IA)

| Fase | Duración | Entregable | Criterio de salida |
|---|---|---|---|
| **0. Fundaciones y spikes** *(esta sesión)* | ~1 sem | Docs, workspace Cargo y pnpm, CI, migraciones v1 probadas, **spike A** (rmcp), **spike B** (sandbox) | Spikes en verde, o plan B activado y documentado |
| **1. MVP gateway** | ~3–4 sem | Hub stdio, sincronización del catálogo, typegen (snapshot tests), búsqueda FTS5+trigram+RRF, `execute` con worker aislado, `workspace`, política y aprobaciones, auditoría, CLI (`init`, `import-config`, `install --client` para los 4 clientes, `sync`, `serve --stdio`, `serve --http` con bearer para claude.ai vía túnel, `stats`), evals v1 | ≤ 2k tokens fijos, recall@5 ≥ 90 %, ahorro medido en Claude Code, Codex y Antigravity |
| **2. Segundo cerebro** | ~4 sem | Memorias, `recall`, `remember` y `primer`, extractores md, código (tree-sitter TS/JS, Rust, Python, Go) y config, grafo con enlaces automáticos, indexado incremental y watcher, export/import Markdown, skills | Recall@5 del cerebro ≥ 85 % |
| **3. Robustez y alcance** | ~3–4 sem | OAuth 2.1 para claude.ai, elicitation, PII, embeddings híbridos (si el eval mejora), extractores CSV, ipynb, PDF, DOCX y git, resources espejo | E2E en los 4 clientes más claude.ai |
| **4. Producto (opcional)** | — | Dashboard web, sincronización en equipo, endurecimiento del sandbox, empaquetado (`cargo-dist`, Homebrew, wrapper npm, `.mcpb`), `@cerebro/client` | Según demanda |

## 12. Riesgos

| Riesgo | Mitigación |
|---|---|
| El modelo escribe código con errores | Firmas concisas, errores "didácticos", sugerencias, `cerebro_call` como fallback y medirlo en el eval |
| `rmcp` frente a la spec 2026-07-28 | Trait de transporte, versiones fijadas, spike A y sidecar TS como plan B |
| Escape del sandbox | Proceso aislado, rlimits, sin capacidades y actualizaciones al día |
| Acciones destructivas por inyección de prompts | Confirmación, auditoría, skills no confiables por defecto, perfil remoto restrictivo |
| El cliente sigue cargando los servidores originales | `install --replace` con backup y un aviso en `cerebro doctor` |
| Formatos de config de cliente que cambian (Codex, Antigravity) | Adaptadores por cliente con tests de fixtures y `--dry-run` con diff |
| La búsqueda falla | Golden set, alias, boost por uso, embeddings en fase 3 |
| Índice de código grande o lento | Incremental con blake3, límites de tamaño, `.cerebroignore` |
| Cerebro degradado (memorias obsoletas) | `status`/`supersedes`, `importance`, `cerebro brain review` |
| Solapamiento con funciones nativas (ToolSearch de Claude Code, tool search + programmatic tool calling de la API de Claude, Code Mode de Cloudflare) | Valor diferencial: **cualquier cliente o modelo**, local, **compone varios servidores** en una ejecución y añade grafo, memoria, política y métricas. Se revisa en cada fase. |

## 13. Coste y valor

- **TCO:** es local, sin hosting, salvo que se quiera claude.ai (túnel gratuito o un VPS pequeño). El coste está en el desarrollo, en mantenerse al día con la spec MCP (unas 2 revisiones al año) y en un gasto pequeño de API para los evals. Los embeddings son locales.
- **Valor (hipótesis que hay que medir):** menos tokens por tarea, más herramientas sin degradar el contexto, y memoria más grafo de código reutilizables en todos los proyectos de ForgeX y sus clientes. Potencial de producto u open source.

---

## 14. Qué ejecutaré al aprobar (Docs + Fase 0)

**Documentación**
1. `docs/PLAN.md`: este plan ampliado.
2. `docs/adr/0001-rust-core-ts-guest.md`, `0002-sandbox-quickjs.md`, `0003-sqlite-source-of-truth-md-export.md`, `0004-generic-proxy-runtime.md` y `0005-lazy-downstream-servers.md`.
3. `docs/sandbox-api.d.ts`: la API completa más un ejemplo de `.d.ts` generado.
4. `docs/meta-tools.md`: los JSON Schema y descripciones exactas de las 5 meta-herramientas, con su presupuesto de tokens.
5. `docs/clients.md`: cómo conectar Claude Code, Claude Desktop, Codex Desktop y Antigravity 2.0 (stdio), y claude.ai (remoto).
6. `README.md`: visión, arquitectura y enlaces.

**Andamiaje y spikes**
7. Workspace Cargo con los 4 crates, `rust-toolchain.toml`, `rustfmt` y `clippy`. Workspace pnpm con `runtime`, `fixtures` y `e2e`.
8. `crates/cerebro-core/migrations/{hub,brain}/0001_init.sql` con todo el esquema de §8, más el módulo `store` que abre `brain.db`, hace `ATTACH` de `hub.db`, aplica pragmas y migraciones. Tests: FTS sincronizado en insert, update, delete y borrado en cascada, y ranking con `bm25()`.
9. **Spike A** (`cerebro-hub` + bin `cerebro`):
   - (a) El cliente rmcp se conecta por stdio al fixture `echo` (TS) y a `@modelcontextprotocol/server-filesystem`, hace `tools/list`, `tools/call` y guarda el catálogo en `hub.db`.
   - (b) `cerebro serve --stdio` y `serve --http` (bearer) exponen las 5 meta-herramientas. `search_tools` responde sobre el catálogo real; `execute`, `recall`, `remember` y `primer` son stubs.
   - (c) E2E en `packages/e2e` con el SDK TS oficial: `tools/list` devuelve exactamente 5 herramientas, se mide el tamaño de su definición y se prueban ambos transportes.
10. **Spike B** (`cerebro-sandbox`): oxc quita los tipos y rquickjs ejecuta con límite de memoria y timeout por interrupt handler. La función de host async `__call` se resuelve desde tokio y el prelude mínimo de `@cerebro/runtime` usa `Proxy`. Tests:
    - un bucle infinito acaba en `TIMEOUT`;
    - una bomba de memoria acaba en OOM sin tumbar el host;
    - TS con tipos se ejecuta;
    - `return` se serializa a JSON;
    - la salida se recorta;
    - no existen `fetch`, `require` ni `process`.
11. `.github/workflows/ci.yml`: `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `pnpm -r build/test` y `tsc --noEmit`.
12. Commit o commits descriptivos y `git push -u origin ccr-f6dcb0c5-1kktmp`. **Sin PR** salvo que lo pidas.
13. Si un spike no pasa: documentar el hallazgo en su ADR, aplicar el plan B si cabe en la sesión, o dejarlo reportado tal cual. **No se marca como hecho algo que no está verificado.**

## 15. Verificación

- **Esquema:** `cargo test -p cerebro-core` (migraciones sobre BD temporal, triggers FTS, cascada, `bm25()`). Comprobación rápida extra con `python3 -I` + `sqlite3` 3.53.1 (FTS5 y trigram disponibles).
- **Tipos:** `tsc --noEmit --strict` sobre `docs/sandbox-api.d.ts` con un ejemplo de uso.
- **Spikes:** `cargo test --workspace`, `pnpm -r build && pnpm -r test` (E2E contra el binario compilado) y `cargo clippy --workspace -- -D warnings`.
- **Manual (para ti, tras la sesión):** `claude mcp add cerebro -- cerebro serve --stdio` y comprobar en `/mcp` que aparecen 5 herramientas; lo mismo con `cerebro install --client codex|antigravity` en la fase 1.

## 16. Fuentes

- Anthropic, "Code execution with MCP": https://www.anthropic.com/engineering/code-execution-with-mcp
- Cloudflare, Code Mode: https://blog.cloudflare.com/code-mode-mcp/ · https://blog.cloudflare.com/code-mode/
- Spec MCP 2026-07-28: https://modelcontextprotocol.io/specification/2026-07-28/changelog
- Codex MCP: https://developers.openai.com/codex/mcp · Antigravity MCP: https://antigravity.google/docs/mcp
- claude.ai, conectores remotos: https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp
- rmcp: https://docs.rs/crate/rmcp/3.10.0 · sqlite-vec: https://github.com/asg017/sqlite-vec/releases
