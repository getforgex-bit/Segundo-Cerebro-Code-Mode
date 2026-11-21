# Protocolo multi-sesión

## 1. Roles

- **Coordinador:** la sesión `session_018vGgtDkbBcKozrRZMWdwQz`, rama `ccr-f6dcb0c5-1kktmp`.
  - Da las órdenes, prioriza y resuelve conflictos entre líneas.
  - Toma las decisiones de arquitectura y aprueba los cambios de contrato.
  - Revisa e integra las ramas y mantiene `STATUS.md`.
  - Se encarga del **razonamiento pesado** que le consultéis.
- **Sesiones de trabajo (WS1–WS4):** ejecutan su brief (`coordination/ws/WSx-*.md`). Son autónomas dentro de su alcance y **consultan** cuando una decisión las supera (ver §4).

## 2. Ramas y git

- Cada WS trabaja **solo** en su rama (`ws/ws1-fork`, `ws/ws2-store`, `ws/ws3-hub`, `ws/ws4-server`), creada a partir de `ccr-f6dcb0c5-1kktmp`.
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
| `crates/cerebro_store/`, `crates/cerebro_search/`, `docs/cerebro/adr/0004-*`, `docs/cerebro/schema.md` | WS2 |
| `crates/cerebro_hub/`, `crates/cerebro_policy/`, `packages/fixtures/`, `docs/cerebro/adr/0005-*`, `docs/cerebro/threat-model.md` | WS3 |
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
- `cargo clippy -p <tu crate> -- -D warnings` sin avisos en **tus** crates.
- `cargo fmt` aplicado.
- Documentación del hito actualizada (doc comments y, si aplica, tu doc en `docs/cerebro/`).
- Reporte enviado con la verificación **real**. Si algo no se verificó, se dice.

## 7. Integración

1. La WS envía `[LISTO PARA INTEGRAR]` y hace push a su rama.
2. El coordinador revisa, hace merge en `ccr-f6dcb0c5-1kktmp`, ejecuta las pruebas de integración y hace push.
3. El coordinador responde `[INTEGRADO] <sha>` e indica a las demás WS si deben hacer merge.

Orden previsto: WS2 (API real del store) → WS3 (hub y política) → WS4 (servidor y E2E) → WS1 (CI final). WS1 puede integrar documentos antes.
