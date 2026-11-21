# Protocolo multi-sesión

> **Alcance de las secciones.**
> - §1–§7: comunicación **dentro del Equipo A** (sesiones Claude con `send_message`). Nacieron en la Fase 0 y siguen vigentes.
> - §8–§10: **entre equipos** (A Claude Code ↔ B Antigravity). Issues y PRs, relevo y ramas.
> - Roles y propiedad por rutas: `coordination/TEAMS.md`. Desde la Fase 1, sustituye a la tabla de §3 cuando haya conflicto entre ambas.

## 1. Roles

- **Coordinador:** la sesión `session_018vGgtDkbBcKozrRZMWdwQz`, rama `ccr-f6dcb0c5-1kktmp`.
  - Da las órdenes, prioriza y resuelve conflictos entre líneas.
  - Toma las decisiones de arquitectura y aprueba los cambios de contrato.
  - Revisa e integra las ramas y mantiene `STATUS.md`.
  - Se encarga del **razonamiento pesado** que le consultéis.
- **Sesiones de trabajo (WS1–WS4):** ejecutan su brief (`coordination/ws/WSx-*.md`). Son autónomas dentro de su alcance y **consultan** cuando una decisión las supera (ver §4).

## 2. Ramas y git

- Fase 0 (cerrada): cada WS trabajó **solo** en su rama (`ws/ws1-fork`, `ws/ws2-store`, `ws/ws3-hub`, `ws/ws4-server`), creada a partir de `ccr-f6dcb0c5-1kktmp`. **Desde la Fase 1, las ramas siguen §10** (troncal `develop`, `a/<issue>-<slug>`).
- **Prohibido:**
  - hacer push a otra rama, incluida la de integración;
  - `--force`;
  - reescribir historia publicada;
  - abrir PRs;
  - tocar el remoto `upstream`.
- Commits pequeños y descriptivos, en español o inglés, con el prefijo de la WS. Ej.: `ws2: tablas FTS de tools con triggers`.
- Para traer lo integrado: `git fetch origin ccr-f6dcb0c5-1kktmp && git merge --no-edit FETCH_HEAD`. Hazlo cuando el coordinador lo indique, o antes de pedir integración.
- **`Cargo.lock`:** si hay un conflicto, quédate con la versión de integración y ejecuta `cargo build` para regenerarlo. No lo edites a mano.
- **Dependencias:** declara las de cada crate **en su propio `Cargo.toml`**. No modifiques `[workspace.dependencies]` del `Cargo.toml` raíz (es de upstream). Si necesitas `serde`, `serde_json`, `anyhow`, `thiserror`, `tokio` o `tracing`, puedes usar `workspace = true`. **No uses `rmcp` del workspace**, que es la 1.8 de pctx: declara `rmcp = { version = "3.11", default-features = false, features = [...] }`.

## 3. Propiedad de archivos

Solo el dueño edita. Si necesitas cambiar algo ajeno, **consulta**.

| Ruta | Dueño |
|---|---|
| `crates/cerebro_types/`, `coordination/`, `README.md`, `packages/package.json`, `packages/pnpm-workspace.yaml`, `docs/cerebro/adr/0001-*`, `docs/cerebro/adr/0006-*` | Coordinador |
| `crates/pctx*/` (parches `CEREBRO-PATCH`), `.github/workflows/cerebro-*.yml`, `docs/cerebro/UPSTREAM.md`, `docs/cerebro/adr/0002-*`, `docs/cerebro/adr/0003-*` | WS1 (**cada parche a pctx requiere aprobación previa del coordinador**) |
| `crates/cerebro_store/`, `crates/cerebro_search/`, `docs/cerebro/adr/0004-*`, `docs/cerebro/schema.md`, `docs/cerebro/threat-model.md` | WS2 |
| `crates/cerebro_hub/`, `crates/cerebro_policy/`, `packages/fixtures/`, `docs/cerebro/adr/0005-*` | WS3 |
| `crates/cerebro/`, `packages/e2e/`, `docs/cerebro/meta-tools.md`, `docs/cerebro/clients.md`, `docs/cerebro/sandbox-api.d.ts` | WS4 |
| `docs/*.md`, `README.pctx.md` y el resto de archivos de upstream | Nadie: son de upstream y no se tocan salvo un parche aprobado |

`packages/fixtures/` es de WS3, pero WS4 lo consume. Si WS4 necesita un fixture nuevo, se lo pide a WS3 a través del coordinador.

## 4. Cuándo consultar al coordinador

**Consulta (y espera respuesta)** antes de:
1. Cambiar un contrato (`cerebro_types` o `CONTRACTS.md`).
2. Parchear código de upstream (`crates/pctx*`).
3. Tomar una decisión de arquitectura o de seguridad no cubierta por el plan, o que lo contradiga.
4. Añadir una dependencia pesada (más de unos 30 crates transitivos, compilación nativa, o descargas en tiempo de build).
5. Encontrar un bloqueo que te impida avanzar más de unos 20 minutos.
6. Ver que el plan es incorrecto o hay una alternativa claramente mejor.

**No consultes** por detalles de implementación dentro de tu alcance: decide tú y anótalo en tu reporte.

**Mientras esperas respuesta,** sigue con otra parte de tu brief que no dependa de la consulta.

## 5. Cómo enviar mensajes

Usa la herramienta `send_message` del servidor MCP `claude-code-remote`, con `session_id = "session_018vGgtDkbBcKozrRZMWdwQz"`. Si no la tienes cargada, búscala con ToolSearch (`send_message`).

Formato (primera línea = tipo):

```
[CONSULTA WSx] <título corto>
Contexto: <qué estás haciendo, 2–4 líneas>
Pregunta: <decisión concreta que necesitas>
Opciones: A) … B) … (y tu recomendación)
Bloquea: <sí/no; qué sigues haciendo mientras>
```

```
[REPORTE WSx] <hito>
Hecho: <lista breve, con rutas>
Verificado: <comandos ejecutados y resultado real; no afirmes nada que no ejecutaste>
Pendiente: <lista>
Decisiones tomadas: <lista breve>
Rama/commit: ws/wsx-… @ <sha corto>
```

```
[LISTO PARA INTEGRAR WSx] <sha corto>
Qué incluye: …
Cómo verificar: <comandos>
Riesgos: …
```

Envía un `[REPORTE]` al terminar cada hito de tu brief y **siempre** uno final, aunque termines con bloqueos. El coordinador responde por el mismo canal con `[ORDEN]`, `[RESPUESTA]` o `[INTEGRADO]`.

## 6. Definition of Done (por hito)

- Compila: `cargo build -p <tu crate>`, o `pnpm -r build` en TS.
- Tests propios en verde: `cargo test -p <tu crate>`.
- `cargo clippy -p <tu crate> --all-targets --no-deps -- -D warnings` sin avisos. `--no-deps` evita que se analicen los crates de pctx.
- `cargo fmt` aplicado.
- Documentación del hito actualizada (doc comments y, si aplica, tu doc en `docs/cerebro/`).
- Reporte enviado con la verificación **real**. Si algo no se verificó, se dice.

## 7. Integración

1. La WS envía `[LISTO PARA INTEGRAR]` y hace push a su rama.
2. El coordinador revisa, hace merge en `ccr-f6dcb0c5-1kktmp`, ejecuta las pruebas de integración y hace push.
3. El coordinador responde `[INTEGRADO] <sha>` e indica a las demás WS si deben hacer merge.

Orden previsto: WS2 (API real del store) → WS3 (hub y política) → WS4 (servidor y E2E) → WS1 (CI final). WS1 puede integrar documentos antes.

## 8. Issues y PRs entre equipos (A ↔ B)

**Canales:**
- Dentro de A: `send_message` (§5).
- Dentro de B: el tablero del swarm de Antigravity.
- **Entre equipos: GitHub Issues y PRs.**
- Decisiones duraderas: ADR (`docs/cerebro/adr/`) y `CONTRACTS.md`.
- Estado global: `STATUS.md` (lo mantiene Opus).

**Issues:**
- Plantillas: `.github/ISSUE_TEMPLATE/cerebro-task.md`, `cerebro-contract-change.md` y `cerebro-decision.md`.
- Cada Issue lleva: objetivo, contexto mínimo (enlaces, no texto copiado), equipo y responsable (agente concreto en el cuerpo), dependencias, rutas y contratos afectados, criterios de aceptación, estado, bloqueos, PR y resultado.
- Etiquetas:

| Grupo | Valores |
|---|---|
| Equipo | `team:a`, `team:b` |
| Responsable | `owner:opus`, `owner:neon`, `owner:sonnet`, `owner:hydrogen` |
| Tipo | `type:task`, `type:contract-change`, `type:decision`, `type:review`, `type:handoff`, `type:eval`, `type:blocker`, `type:coordination`, y las de GitHub `bug` y `documentation` |
| Estado | `status:ready`, `status:in-progress`, `status:blocked`, `status:review` (cerrado = hecho) |

**Reglas:**
1. Cada Issue tiene **un** responsable principal. Un equipo no toma un Issue activo del otro sin acuerdo en el propio Issue.
2. Las dependencias cruzadas se escriben en ambos Issues ("Depende de #N" / "Bloquea a #M").
3. Un cambio de contrato requiere **Issue `type:contract-change`** y el visto bueno explícito de Opus y de Neon antes del merge.
4. Firma obligatoria en cada comentario, porque todos usamos la misma identidad de GitHub: `— Opus · Equipo A`, `— Sonnet-B · Equipo A`, `— Neon · Equipo B`, `— Hydrogen @indexer · Equipo B`. Los comentarios escritos por Claude terminan con el pie `Generated by Claude Code`.
5. **Anti-ruido:** el Issue guarda decisiones, evidencia y estado, nunca conversaciones. Los logs de más de 30 líneas se resumen.
6. Desacuerdos: TEAMS.md §4, con un máximo de 2 rondas.
7. Al cerrar, el comentario final incluye las **métricas para `AGENT-EVALS.md`**: modelo, éxito/CI, rondas de revisión, defectos, tiempo y coste o cuota.

**PRs:**
- Origen `a/<issue>-<slug>` o `b/<issue>-<slug>`, destino **`develop`**.
- Cuerpo: `Issue #N · Equipo X · Contratos tocados: … · Verificación ejecutada: <comandos y resultado real>`.
- Requisitos: CI `cerebro-ci` en verde. **Revisión cruzada obligatoria** (por el otro equipo) si el PR toca contratos, política, aprobaciones, sandbox o parches pctx, escritura de configs del usuario (`cerebro_clients`) o sincronización del cerebro.
- **El merge lo hace Opus** (merge commit). Si un merge rompe `develop`, se revierte y se reabre el Issue.
- El Equipo A se suscribe a la actividad de los PRs abiertos. Opus revisa Issues y PRs en check-ins programados (~cada 3 h mientras haya trabajo activo).

## 9. Relevo y recuperación de sesiones

Se usa cuando una sesión se interrumpe, agota cuota o contexto, o rota (coordinadores: ~300k tokens en Opus, ~250k en Neon). Generaliza el `[REPORTE] RELEVO` de la Fase 0.

1. Commit de todo lo pendiente (`<prefijo>: WIP <qué>`) y push a la rama de trabajo.
2. Comentario en el Issue con la etiqueta `type:handoff` (en el Equipo A, además, `[REPORTE] RELEVO` por `send_message`):

```
RELEVO · <Issue #N> · <agente> · <equipo>
Rama / último commit: <rama> @ <sha>
Sin confirmar: <nada | lista>
Hecho y verificado: <lista, con los comandos ejecutados y su resultado>
Pendiente: <lista ordenada>
Errores conocidos: <lista>
Próxima acción recomendada: <una línea>
Requiere autorización: <decisiones abiertas>
```

3. Quien retoma **verifica el estado real** (`git log`, `git status` y los tests del crate) antes de continuar. Se transfieren resúmenes verificables, no conversaciones.

## 10. Ramas

| Rama | Uso | Quién escribe |
|---|---|---|
| `main` | Versiones estables | Solo con aprobación del usuario |
| **`develop`** | Troncal de integración de ambos equipos | Solo Opus (merge de PRs) |
| `a/<issue>-<slug>` | Trabajo del Equipo A | Sesiones del Equipo A |
| `b/<issue>-<slug>` | Trabajo del Equipo B | Neon y los Hydrogen |
| `ws/*`, `ccr-*` | Histórico de la Fase 0 | Solo lectura |
| `astra*` | Fuera de alcance | **Nadie**: no tocar, fusionar ni borrar |

- Para actualizar una rama de trabajo: `git fetch origin develop && git merge --no-edit FETCH_HEAD`. Nunca `rebase` de historia publicada ni `--force`.
- Conflictos en `Cargo.lock` o `packages/pnpm-lock.yaml`: toma la versión de `develop` y regenera (`cargo build -p <crate>` / `pnpm -C packages install`).
- Propiedad temporal de archivos compartidos: se anuncia en el Issue ("tomo `X` hasta el PR #N") y se libera al hacer merge.

