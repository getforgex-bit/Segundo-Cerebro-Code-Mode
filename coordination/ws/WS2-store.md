# WS2: Store SQLite y búsqueda

**Rama:** `ws/ws2-store` · **Coordinador:** `session_018vGgtDkbBcKozrRZMWdwQz` · **Lee antes:** `../PROTOCOL.md`, `../CONTRACTS.md` (§1, §2, §4 y §6), `../PLAN.md` §V2.6, §V2.7-0c y Anexo A §8, y `crates/cerebro_types/src/lib.rs`.

## Objetivo

La persistencia y la búsqueda del catálogo sobre las que trabajan el hub y el servidor. Es la **primera pieza que se integra**, así que su API pública debe quedar estable pronto.

## Alcance

**Incluye:**

H1. **API estable lo antes posible.** Crea `crates/cerebro_store` y `crates/cerebro_search` con las firmas de `CONTRACTS.md` §2 compilando, aunque la implementación sea mínima. Envía `[LISTO PARA INTEGRAR WS2] H1` para que el coordinador lo integre pronto y WS3 y WS4 dejen sus mocks.

H2. **Migraciones** con `rusqlite_migration` e `include_str!` desde `crates/cerebro_store/migrations/{hub,brain}/0001_init.sql`:
- `hub`: `servers`, `tools`, `tools_fts` (`porter unicode61 remove_diacritics 2`), `tools_trgm` (`trigram`), ambas external content con triggers de insert, update y delete, y `tool_stats`.
- `brain`, esquema completo de la Fase 2 ya en v1 para no migrar dos veces:
  - `memories`, con `visibility` (`team` \| `personal`), `author`, `sync_hash` y `synced_at` (V2.6);
  - `memories_fts` sin porter, con `prefix='2 3'`, y `memories_trgm`;
  - `documents`, `symbols` (con su FTS), `chunks` (con su FTS) y `links` (refs URI);
  - `skills` (con su FTS);
  - `sessions`, `executions`, `tool_calls`, `approvals`, `workspace_kv`, `result_handles` (M6) y `sync_state`;
  - la vista `v_token_savings`.
- Pragmas: WAL, `foreign_keys=ON`, `busy_timeout=5000`. `hub.db` se adjunta como `hub` (`ATTACH`). Cada esquema tiene su propia cadena de migraciones.
- Incluye `CHECK` razonables (enums, `json_valid`).

H3. **`impl Catalog, AuditLog, ApprovalStore for Store`**:
- `replace_server_tools` es atómico y reemplaza las herramientas del servidor; las FTS quedan sincronizadas por triggers.
- Aprobaciones:
  - el token es aleatorio (≥ 128 bits, hex o base32);
  - un solo uso;
  - expiración por `ttl_seconds`;
  - `consume` verifica `tool` y `args_hash`.

H4. **`cerebro_search`**:
- Consulta FTS5 con `bm25()` sobre `tools_fts`, más `tools_trgm` para coincidencias difusas, combinadas con **RRF** (k=60).
- Filtro por servidor y límite (por defecto 8, máximo 25).
- La consulta se sanea: escapa la sintaxis FTS y parte camelCase y snake_case.
- `format_hits` exactamente como en `CONTRACTS.md` §4.

H5. **`docs/cerebro/schema.md`** (diagrama de tablas y por qué de cada índice) y **ADR `0004-sqlite-md-sync.md`** (SQLite como fuente local + Markdown en git para el equipo, con el diseño de V2.6).

**No incluye:** el sincronizador Markdown ni los extractores (Fase 2), ni embeddings.

## Criterios de aceptación (tests en `cargo test -p cerebro_store -p cerebro_search`)

- Las migraciones se aplican en una BD nueva y son idempotentes al reabrir.
- Las FTS siguen sincronizadas tras insert, update y delete **y** tras un borrado en cascada (borrar un servidor borra sus herramientas y sus filas FTS).
- Búsqueda:
  - `"create issue"` encuentra `github.create_issue` antes que `github.list_issues`;
  - `"isue"` encuentra algo vía trigram;
  - `server` filtra.
- Aprobaciones: válida, expirada, reutilizada, `args_hash` distinto y `tool` distinto.
- `format_hits` con los 3 niveles de detalle, en tests snapshot (`insta`) o literales.
- `clippy -D warnings` limpio.

## Consultas previstas

- Si el esquema del Anexo A §8 tiene huecos o contradicciones con V2.6 o V2.7.
- Si `Store` necesita ser async (por defecto no: `Mutex<Connection>` y llamadas cortas; el hub lo envuelve en `spawn_blocking` si hace falta).

## Primer paso

Crea los dos crates con las firmas, haz commit y push, y avisa con `[LISTO PARA INTEGRAR WS2] H1`. Después, las migraciones.
