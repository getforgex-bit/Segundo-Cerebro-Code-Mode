# Modelo de amenazas de Cerebro (Fase 0)

- **Estado:** borrador 1 (2026-11-21) · **Dueño:** WS2 (traspasado desde WS3) · **Alcance:** Fase 0; las fases 2 y 3 se tratan como amenazas previstas.
- **Método:** STRIDE reducido, por superficie. Solo se cuentan como controles los que **existen en código**; lo demás figura como pendiente.

## Cómo leer los estados

| Marca | Significado |
|---|---|
| **Integrado** | Está en `ccr-f6dcb0c5-1kktmp` y tiene tests. |
| **Rama** | Existe en la rama de una WS pero aún no está integrado (se cita la rama). Puede cambiar. |
| **Contrato** | Está acordado en `coordination/CONTRACTS.md` o en el plan, sin código. |
| **Pendiente** | No existe todavía. |

> Al redactar este documento, `cerebro_hub` es un esqueleto sin lógica (`ws/ws3-hub`), así que **la redacción de argumentos, el arranque de procesos downstream y el flujo de aprobación completo no existen aún como código**. Donde el documento los menciona, son de tipo *Contrato*.

## 1. Activos

| Activo | Dónde vive | Por qué importa |
|---|---|---|
| **A1. Credenciales de servidores downstream** (tokens, claves en `env`) | `env` de cada servidor en `.cerebro/config.json` (`crates/cerebro/src/config.rs`, rama `ws/ws4-server`); entorno del proceso hijo | Dan acceso a GitHub, bases de datos, etc. |
| **A2. Datos que devuelven las herramientas** | Resultados en memoria del sandbox; `result_handles` (M6, futuro) | Pueden ser privados (correo, código, datos de clientes) |
| **A3. El cerebro** (memorias, índice de código, skills) | `brain.db` (`crates/cerebro_store/migrations/brain/0001_init.sql`), Markdown en git (ADR-0004) | Decisiones, convenciones e incidentes del equipo; código fuente |
| **A4. Tokens de aprobación** | Tabla `approvals` | Autorizan acciones destructivas |
| **A5. Auditoría** | `tool_calls`, `executions` | Evidencia de qué hizo la IA; no debe filtrar secretos ni poder borrarse en silencio |
| **A6. Catálogo** | `hub.db` (`servers`, `tools`) | Lo que la IA cree que puede hacer; si se envenena, dirige a la IA |
| **A7. Disponibilidad del servidor** | Proceso `cerebro` | Un solo proceso V8 por servidor; su caída corta al cliente |
| **A8. Máquina del usuario** | Procesos hijo con los privilegios del usuario | Los servidores downstream ejecutan código arbitrario |

## 2. Actores

| Actor | Confianza | Capacidad |
|---|---|---|
| **El usuario** | Total | Configura servidores y política; aprueba |
| **La IA cliente** | **Parcial**: sigue instrucciones del contexto, que puede estar contaminado | Escribe el TypeScript de `execute_typescript`; elige `approvals` |
| **Servidor downstream malicioso o comprometido** | **Nula** | Controla nombres, descripciones, esquemas, anotaciones y resultados de sus herramientas; corre como proceso local |
| **Contenido no confiable en los resultados** (issues, páginas, correos) | **Nula** | Inyección de prompts hacia la IA |
| **Otro proceso local / otro usuario del equipo** | Baja | Lee o escribe `.cerebro/` si los permisos lo permiten |
| **Colaborador del repo** | Media | Puede aportar `.cerebro/brain/*.md` y skills (Fase 2) |
| **Atacante remoto** (Fase 3, claude.ai) | **Nula** | Alcanza el endpoint HTTP expuesto |

## 3. Superficies y límites de confianza

```
 cliente MCP ──stdio──▶ cerebro (servidor MCP) ──▶ sandbox Deno ──callbacks──▶ hub ──▶ política ──▶ servidores downstream (stdio)
 (usuario + IA)              │                                                    │            │
                             └──────── store: brain.db + hub.db ◀──── auditoría ──┴─ aprobaciones
```

S1 sandbox · S2 puente de callbacks · S3 hub · S4 política · S5 aprobaciones · S6 auditoría y redacción · S7 SQLite en disco · S8 catálogo y búsqueda · S9 skills (Fase 2) · S10 sincronización Markdown (Fase 2) · S11 modo remoto (Fase 3).

## 4. Amenazas y controles por superficie

Formato: **amenaza (STRIDE)** → controles existentes → pendientes.

### S1. Sandbox Deno (código que escribe la IA)

Hallazgos de WS1 (`crates/cerebro_spike_pctx/tests/spike.rs`, rama `ws/ws1-fork`) y del código de pctx.

| Amenaza | Existente | Pendiente |
|---|---|---|
| **E: salir al sistema** (red, ficheros, entorno, procesos) | El runtime solo carga `pctx_runtime_snapshot` (`crates/pctx_executor/src/lib.rs:381`), sin `deno_fetch`/`deno_net`/`deno_fs`; `NoopModuleLoader` rechaza `import()` (`lib.rs:393`). Test `sandbox_has_no_network_nor_fs`: `fetch`, `WebSocket`, `Deno.connect`, `Deno.readTextFile` no existen y `import("file:///…")` e `import("https://…")` fallan. **Rama** | Mantener el test como guardián de regresión en CI; revisar cada actualización de `deno_core` |
| **D: bucle infinito** | Watchdog con timeout en `pctx_executor::execute` (`lib.rs:431-473`); el test `infinite_loop_is_stopped_by_timeout` lo verifica. **Rama** | `CodeMode::execute_typescript` **no expone el timeout** (siempre 120 s, según WS1), por lo que `limits.timeout_ms` de `.cerebro/config.json` no se aplica todavía: requiere parche `CEREBRO-PATCH` aprobado |
| **D: agotar memoria** | Ninguno | **Sin límite de heap**: según el test `memory_bomb_kills_the_host_process`, V8 aborta **todo el proceso `cerebro`**. Un código de la IA (o inyectado) puede tumbar el servidor. Parche `CEREBRO-PATCH` con `near_heap_limit_callback` pendiente |
| **D: bloqueo de otras ejecuciones** | `V8_MUTEX` serializa las ejecuciones (CONTRACTS §2), lo que evita carreras pero hace que una ejecución lenta bloquee a las demás hasta su timeout | Pool de workers (Fase 1) |
| **D: salida enorme** (`console.log`, retorno) | `limits.max_result_bytes` en la config (`crates/cerebro/src/config.rs`, **Rama**) y campo `truncated` en la salida de `execute_typescript` (CONTRACTS §3) | Límite de `stdout`/logs y de llamadas por ejecución; truncado real aún no verificado |
| **T/E: evadir la política llamando a los servidores sin pasar por el hub** | Diseño: todas las herramientas se registran como callbacks, no con `add_mcp` (PLAN V2.5); el sandbox no tiene red para hablar con los servidores | Test E2E que pruebe que no hay otra vía |

### S2. Puente de callbacks (sandbox → hub)

| Amenaza | Existente | Pendiente |
|---|---|---|
| **S/T: argumentos malformados o hostiles hacia las herramientas** | Cada herramienta tiene `input_schema` y el type-check de pctx corre siempre antes de ejecutar (`type_check_always_runs_and_timing`, **Rama**) | Validar los argumentos contra el JSON Schema en el hub antes de llamar (los tipos TS no existen en runtime) |
| **I: fuga de detalles en errores** | Los errores viajan como `CerebroErrorPayload` con código estable (`crates/cerebro_types/src/lib.rs`, **Integrado**) | Revisar que `message` no incluya rutas, tokens ni stderr del servidor |
| **R: perder el error por `console.error` previo** | `parse_from` más lectura de `trace.events[]` (CONTRACTS §2, **Contrato**) | Implementar y probar en WS4 |
| **T: callbacks que sobreviven a la ejecución** | Los futures se lanzan en el runtime principal (`main_handle.spawn`) (CONTRACTS §2, **Contrato**) | Test de que un timeout no deja procesos hijos huérfanos |

### S3. Hub (procesos downstream)

| Amenaza | Existente | Pendiente |
|---|---|---|
| **E: un servidor malicioso ejecuta código en la máquina** | Es inherente: se lanza con el comando de la config del usuario. Solo el usuario añade servidores (`DownstreamServer`, CONTRACTS §2) | Sin sandbox de proceso (seccomp/landlock, Fase 4). Documentar que añadir un servidor equivale a ejecutar su código |
| **I: robo de credenciales entre servidores** | `env` es por servidor (`DownstreamServer.env`, **Contrato**); el sandbox no ve credenciales (PLAN §9) | Arrancar los hijos con entorno mínimo (no heredar el del proceso `cerebro`); soporte de referencias a variables en lugar de valores en claro |
| **D: servidor colgado o que no responde** | `HubConfig.call_timeout` e `idle_shutdown` (**Contrato**) | Implementar; probar que un servidor caído no falla el `sync` entero (`SyncReport.servers_failed`) |
| **S: suplantación/colisión de nombres** (A define lo mismo que B) | Deduplicación por (`ts_namespace`, `ts_name`) y rechazo de namespaces PascalCase repetidos (CONTRACTS §2, **Contrato**). El catálogo tiene `UNIQUE(server, name)` (`migrations/hub/0001_init.sql`, **Integrado**) | Implementar en el hub; sin ello, dos nombres que colisionan rompen **toda** la ejecución (TS2300) |
| **T: *rug pull*** (el servidor cambia sus herramientas después de aprobarlas) | El catálogo guarda `schema_hash` por herramienta y `catalog_hash` por servidor (`cerebro_types::ToolRecord`/`ServerRecord`, **Integrado**) y `replace_server_tools` es atómico | **Nadie compara hashes ni avisa de cambios**; una herramienta de solo lectura puede pasar a escribir y heredar `allow`. Pendiente: alerta/reaprobación si cambia el hash de una herramienta ya usada |

### S4. Política (`allow | confirm | deny`)

Código en `crates/cerebro_policy/src/lib.rs` (rama `ws/ws3-hub`, aún sin integrar).

| Amenaza | Existente | Pendiente |
|---|---|---|
| **E: anotaciones mentirosas** (`readOnlyHint` falso para saltarse la confirmación) | Las anotaciones se tratan como pistas; un override `deny`/`confirm` las endurece (nota en el módulo); lo desconocido va a `Confirm` por defecto (`default_unknown`) | **Un servidor hostil puede declarar `readOnlyHint: true` y obtener `Allow`.** El control real es una lista de overrides del usuario y revisar los servidores nuevos. Considerar `default_unknown` para servidores no listados como de confianza |
| **E: anotaciones contradictorias** | CONTRACTS §2 fija: `readOnlyHint` y `destructiveHint` ambos `true` → `Confirm` | **Discrepancia detectada:** el test `read_only_beats_destructive_hint` de la rama `ws/ws3-hub` espera `Allow` (se escribió antes del cambio de contrato). WS3 debe alinearlo |
| **T: política evadida por patrón** | Los overrides se resuelven por clave exacta `server.tool` > `server.*` > `*` (sin expresiones regulares) | Validar la config al arrancar (claves que no existen, `deny_unknown_fields` ya activo en `PolicyConfig`) |
| **E: flujo de datos** (trifecta) | `CallCtx.taint_seen` y `ToolRecord.taint_labels` existen como campos (**Integrado**) y la tabla `tools` los persiste | La regla de taint M5 **no existe**: hoy `Policy::decide` ignora `ctx` (así lo declara el módulo) |

### S5. Aprobaciones

Código: `crates/cerebro_store/src/lib.rs` (`impl ApprovalStore for Store`, **Integrado**, 5 tests).

| Amenaza | Existente | Pendiente |
|---|---|---|
| **S: adivinar un token** | 128 bits de `getrandom` (16 bytes, hex de 32 caracteres) | — |
| **R/T: reutilizar un token** | `consume` es un único `UPDATE … WHERE status IN ('pending','approved') AND expires_at > now` y exige exactamente 1 fila afectada: un solo uso, aun con concurrencia | — |
| **E: usar el token para otra acción** | `consume` verifica `tool` **y** `args_hash`; un intento con otra herramienta o hash no quema el token (test `approval_wrong_tool_or_hash_does_not_burn_token`) | — |
| **E: token caducado** | `expires_at = now + ttl` calculado en SQL; test de caducidad | Limpieza periódica de filas caducadas (hay índice `approvals_expires`) |
| **E: la IA se auto-aprueba** | `approve` no está expuesto en la superficie MCP de la Fase 0 (CONTRACTS §3) | **Hueco importante:** `consume` acepta tokens en estado `pending`, por contrato. Con tokens propios, la IA recibe el token en el error `NEEDS_APPROVAL` y puede reenviarlo en `approvals` sin que un humano lo apruebe fuera de banda. En la Fase 0 la única barrera humana es que el **cliente MCP pida confirmación** al ver la segunda llamada a `execute_typescript`. Cuando exista un canal fuera de banda (MRTR/elicitation o `cerebro approve`), `consume` debe exigir `approved` |
| **T: reutilizar la aprobación en otra ejecución** | El token está ligado a `tool` y `args_hash`, no a `execution_id`: sirve en cualquier ejecución con la misma llamada exacta | Aceptable en la Fase 0 (misma acción, mismos argumentos); evaluar ligarlo a la sesión |
| **I: el hash revela argumentos** | `args_hash` es blake3 de JSON canónico | Si los argumentos son de baja entropía, el hash permite confirmarlos por fuerza bruta: no exponerlo fuera de `approvals`/`tool_calls` |

### S6. Auditoría y redacción

| Amenaza | Existente | Pendiente |
|---|---|---|
| **I: secretos en la auditoría** | El contrato exige `args_redacted` (campos `token/password/secret/key` → `"***"`, comentario en `ToolCallRecord`) y se guarda `args_hash` | **La redacción no existe**: `record_tool_call` guarda tal cual lo que recibe (`crates/cerebro_store/src/lib.rs`). La implementa el hub. Falta redactar también `error` y resultados; la heurística por nombre de campo no cubre secretos dentro de valores |
| **R: repudio** (negar que la IA hizo algo) | `tool_calls` registra servidor, herramienta, decisión, estado y duración; `record_tool_call` no tiene FK a `executions`, así que se registra aunque la ejecución falle (**Integrado**) | No hay cadena de hashes ni firma: quien escribe en `brain.db` puede alterar el historial |
| **T: perder el rastro de una ejecución abortada** | Las llamadas se escriben al terminar cada una | Un abort por OOM (S1) deja la fila de `executions` sin cerrar |

### S7. SQLite en disco

| Amenaza | Existente | Pendiente |
|---|---|---|
| **I/T: otro proceso o usuario lee o altera `.cerebro/`** | Ninguno: `Store::open` usa `create_dir_all` y `Connection::open` con la *umask* del usuario (`crates/cerebro_store/src/lib.rs`) | **`.cerebro/` con permisos 0700 y archivos 0600** al crearlos |
| **T: inyección SQL** | Toda consulta de datos usa parámetros (`params!`). Los nombres de tabla/columna dinámicos son constantes internas | — |
| **T: inyección en la sintaxis FTS** | La consulta se parte en palabras alfanuméricas con `split_words` y cada término va entre comillas; el trigram solo genera subcadenas de esos términos (`crates/cerebro_search/src/lib.rs`, `fts_expr`/`trigram_expr`). El test `hostile_syntax_is_neutralised` prueba `OR`, comillas sin cerrar, `NEAR(` y `; DROP TABLE` | — |
| **D: corrupción o bloqueo** | WAL, `busy_timeout=5000`, `foreign_keys=ON`, CHECK de enumerados y `json_valid`; migraciones idempotentes (test `disk_open_is_idempotent_and_persists`) | Copias de seguridad; `PRAGMA integrity_check` en `doctor` |
| **I: secretos del repo** | — | `.cerebro/brain.db*`, `.cerebro/cache/` y `.cerebro/workspace/` deben ir en `.gitignore` del proyecto (PLAN V2.6); el `.gitignore` de este repo no los lista: lo escribirá la CLI (`cerebro init`), aún no existe |
| **I: datos en claro** | — | Sin cifrado en reposo; aceptado en la Fase 0 |

### S8. Catálogo y búsqueda (envenenamiento de herramientas)

| Amenaza | Existente | Pendiente |
|---|---|---|
| **T/E: inyección en descripciones** (un servidor escribe «ignora las instrucciones anteriores…» en `description`) | `format_hits` recorta la descripción a la primera frase o 100 caracteres y aplana saltos de línea; la firma TS también se aplana a una línea (test `signature_cannot_inject_extra_lines`), de modo que un servidor no puede falsear filas de resultados | Es una **mitigación parcial**: 100 caracteres bastan para una instrucción corta. `get_function_details` y la declaración TS devuelven descripciones completas. Pendiente: delimitar y marcar como no confiable el texto de herramientas que se devuelve a la IA |
| **S: nombres engañosos** (`github.deleteRepo` falso en otro servidor) | La búsqueda muestra siempre `ts_namespace.ts_name`, que incluye el servidor (CONTRACTS §4) | — |
| **D: catálogo gigante** | `limit` ≤ 25 y candidatos acotados (`MAX_LIMIT`, `MIN_CANDIDATES`) | Límite de herramientas por servidor |

### S9. Skills (Fase 2)

| Amenaza | Existente | Pendiente |
|---|---|---|
| **E: una skill del repo ejecuta código sin revisión** | La tabla `skills` tiene `trusted INTEGER DEFAULT 0` (`migrations/brain/0001_init.sql`, **Integrado**) | Todo el flujo: `cerebro skills approve`, sandbox para skills, hash para detectar cambios después de aprobar |

### S10. Sincronización Markdown en git (Fase 2, ADR-0004)

| Amenaza | Existente | Pendiente |
|---|---|---|
| **T: un colaborador (o PR hostil) planta memorias con instrucciones** | `visibility` (`team`/`personal`), `status = 'conflict'` y `sync_hash` en el esquema | Tratar las memorias importadas como **contenido no confiable**: delimitarlas en `cerebro_recall` y revisarlas en PR |
| **I: secretos exportados al repo** | Las memorias `personal` nunca se exportan (diseño) | Redactar secretos antes de exportar; escáner de secretos en el exportador |
| **T: sobrescritura silenciosa** | Diseño: conflicto → `<archivo>.conflict.md` | Implementación y tests |
| **T: ruta maliciosa en frontmatter** (`../..`) | — | Validar que toda ruta exportada/importada quede dentro de `.cerebro/brain/` |

### S11. Modo remoto (Fase 3)

Plan en PLAN §9: perfil `remote` restrictivo (todo lo que escribe pide confirmación, con allowlist de servidores), bearer token, validación de `Origin`, límite de peticiones, TLS en el túnel y OAuth 2.1. **Todo es *Contrato*/Pendiente.** Antes de abrirlo: este modelo de amenazas debe revisarse de nuevo, porque el actor «atacante remoto» pasa de teórico a real y la IA ya no es el único origen de código.

## 5. El «trifecta letal» y la mitigación M5

**Trifecta:** una misma ejecución combina (1) **datos privados** (un `read_file`, un correo), (2) **contenido no confiable** (un issue o una página que puede contener instrucciones) y (3) un **canal de salida** (crear un issue, enviar un correo, publicar). Si una instrucción inyectada en (2) hace que la IA reenvíe (1) por (3), los datos salen aunque cada llamada individual pareciera inocua.

**Estado hoy:**
- La política decide **por herramienta**, no por flujo de datos (`Policy::decide` ignora `ctx`).
- El sandbox sin red evita que el *código* exfiltre por sí mismo; el canal de salida son solo las herramientas, y todas pasan por el hub (diseño).
- Las etiquetas `private-data`, `untrusted-input` y `external-write` viven en `ToolRecord.taint_labels` y se guardan en `hub.tools.taint_labels`; el acumulado de la ejecución en `CallCtx.taint_seen`. Son campos listos, **sin lógica**.

**Cómo lo mitigará M5 (Fase 2):**
1. Cada herramienta lleva etiquetas, por anotaciones y por configuración del usuario (el servidor no debe poder quitarse etiquetas: la config del usuario manda).
2. El hub acumula las etiquetas de las llamadas ya hechas en la ejecución (`taint_seen`).
3. Si la llamada actual aporta la tercera etiqueta que completa el trío, `Policy::decide` devuelve `Confirm` (o `Deny` según perfil), aunque la herramienta fuera `allow`.
4. Limitaciones previstas: el flujo se cuenta por **ejecución**, no entre ejecuciones (la IA puede partir el ataque en dos llamadas a `execute_typescript`, o el dato puede quedar en su contexto); herramientas sin etiquetar quedan fuera. Por eso M5 reduce el riesgo, no lo elimina.

## 6. Pendientes ordenados por riesgo

| # | Pendiente | Riesgo que cubre | Dueño probable |
|---|---|---|---|
| 1 | Límite de heap de V8 (`CEREBRO-PATCH`) | Un código malicioso o con error tumba el proceso | WS1 (con aprobación) |
| 2 | Timeout configurable en `CodeMode` | `limits.timeout_ms` no se aplica | WS1 |
| 3 | Redacción de argumentos, errores y resultados en el hub | Secretos en `tool_calls` | WS3 |
| 4 | `.cerebro/` en 0700/0600 y `.gitignore` | Lectura por otros usuarios; secretos al repo | WS2 (permisos) / WS4 (`init`) |
| 5 | `consume` exige `approved` cuando exista canal fuera de banda (MRTR/CLI) | Auto-aprobación por la IA | WS3 / coordinador |
| 6 | Alinear `readOnly`+`destructive` → `Confirm` en el código de `cerebro_policy` | Tool engañosa heredando `Allow` | WS3 |
| 7 | Alertar si cambia `schema_hash` de una herramienta ya usada | *Rug pull* | WS3 |
| 8 | Validar argumentos contra `input_schema` en el hub | Argumentos fuera de contrato | WS3 |
| 9 | Entorno mínimo para los procesos hijos | Fuga de credenciales entre servidores | WS3 |
| 10 | Taint M5, PII, MRTR | Trifecta, datos personales, UX de aprobación | Fase 2/3 |

## 7. Riesgos residuales aceptados en la Fase 0

1. **Añadir un servidor equivale a ejecutar su código** con los privilegios del usuario; no hay aislamiento de procesos.
2. **Anotaciones MCP no verificables:** un servidor hostil puede declararse de solo lectura. La mitigación es que el usuario revise los servidores y use overrides.
3. **Aprobación humana dependiente del cliente:** sin canal fuera de banda, la única confirmación real es la del cliente MCP.
4. **Sin límite de heap hasta el parche:** una ejecución puede abortar el proceso. Se acepta porque el cliente reinicia el servidor stdio; se pierde la ejecución en curso.
5. **Una ejecución a la vez** (`V8_MUTEX`): una ejecución lenta retrasa a las demás hasta su timeout.
6. **Sin cifrado en reposo ni auditoría a prueba de manipulación:** quien tenga acceso de escritura a `.cerebro/` puede leer y alterar el historial.
7. **Contenido no confiable llega a la IA** dentro de los resultados; solo se mitiga con el recorte de tamaño y la confirmación de lo destructivo. M5 llega en la Fase 2.
8. **Una sola máquina, un solo usuario:** la Fase 0 no modela atacantes remotos ni equipos grandes.

## 8. Cómo mantener este documento

- Cada vez que un control pase de *Contrato* o *Pendiente* a *Integrado*, actualizar la fila **con la ruta del código y el test**.
- No añadir controles que no tengan código o un test que los demuestre.
- Revisar completo al iniciar la Fase 2 (skills, sincronización Markdown) y la Fase 3 (remoto).
