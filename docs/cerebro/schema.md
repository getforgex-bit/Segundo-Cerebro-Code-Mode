# Esquema SQLite de Cerebro

Fuente de verdad: las migraciones de `crates/cerebro_store/migrations/{hub,brain}/0001_init.sql`. Este documento explica su forma y el porqué de cada índice; si difieren, manda el SQL.

## Dos archivos, dos cadenas de migraciones

| Archivo | Esquema | Contenido |
|---|---|---|
| `.cerebro/brain.db` | `main` | Memorias, índice del proyecto, auditoría, aprobaciones |
| `.cerebro/hub.db` | `hub` (ATTACH) | Catálogo de servidores y herramientas downstream |

- `PRAGMA user_version` es **por archivo**, así que cada base tiene su propia cadena de `rusqlite_migration`. El hub se migra en una conexión aparte **antes** del `ATTACH`; brain se migra sobre la conexión principal.
- Pragmas: `busy_timeout=5000`, `foreign_keys=ON`, WAL (solo en disco).
- En memoria (tests) el hub usa una URI `file:cerebro-hub-<pid>-<n>?mode=memory&cache=shared`; el `Store` conserva la conexión de migración para que la BD no desaparezca.
- Los triggers y las FTS de cada base solo referencian objetos de su propio esquema. Por eso la vista `v_token_savings` usa únicamente tablas de brain.
- Tiempos: RFC 3339 UTC con milisegundos (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`). Al ser texto con formato fijo, se comparan como cadenas (`expires_at > now`).

## hub.db

```
servers (name PK) ──< tools (id, UNIQUE(server,name)) ──> tools_fts / tools_trgm (external content)
        └────────────< tool_stats (PK server,name)
```

- **`servers`**: estado y metadatos; el comando y los secretos viven en la config, no aquí.
- **`tools`**: `ON DELETE CASCADE` desde `servers`.
  - `UNIQUE(server, name)` permite el upsert `ON CONFLICT DO UPDATE`, que conserva el `id` (y por tanto el `rowid` de las FTS).
  - `tools_server_ts_name (server, ts_name)`: resolución de `namespace.fn` en `get_function_details` y en los callbacks.
  - `search_text`: servidor + `name` + `ts_name` + título partidos en palabras (snake/camel), para que `createIssue` encuentre `create_issue`.
  - `available`: la herramienta existe en el servidor ahora mismo; la búsqueda filtra `available = 1`.
- **`tools_fts`** (`porter unicode61 remove_diacritics 2`): BM25 con stemming inglés, porque las descripciones MCP suelen estar en inglés. Columnas: `name, title, description, search_text`.
- **`tools_trgm`** (`trigram`): coincidencias difusas (erratas, subcadenas).
- Ambas son *external content* de `tools`; los triggers `tools_ai/ad/au` las mantienen sincronizadas, incluso en borrados en cascada (el `DELETE` de un servidor dispara `tools_ad` por cada fila).
- **`tool_stats`**: clave natural `(server, name)` para sobrevivir a un reemplazo de herramientas.

## brain.db

| Grupo | Tablas | Notas |
|---|---|---|
| Memorias | `memories`, `memories_fts`, `memories_trgm` | `uid` ULID único; `kind` ∈ decision, convention, fact, note, task, question, glossary, incident, preference, summary; `status` ∈ active, superseded, archived, conflict; `visibility` ∈ team (por defecto), personal; `author`, `sync_hash`, `synced_at` para la sincronización Markdown (ADR-0004) |
| Índice de proyecto | `documents`, `symbols`(+fts), `chunks`(+fts) | `chunks.symbol_id` es `ON DELETE SET NULL`; borrar un documento arrastra sus símbolos y chunks |
| Grafo | `links` | refs URI (`memory:`, `file:`, `symbol:`, `skill:`, `tool:`, `commit:`); `UNIQUE(src_uri, dst_uri, rel)` hace idempotente el reindexado |
| Skills | `skills`, `skills_fts` | `trusted` 0/1, **por defecto 0** |
| Ejecución | `sessions`, `executions`, `tool_calls`, `approvals`, `result_handles` | ver abajo |
| Estado | `workspace_kv`, `sync_state` | clave-valor |
| Vista | `v_token_savings` | bytes que no pasaron por el modelo (`raw_bytes_processed − result_bytes`) y tokens aproximados (÷4) |

- Las FTS de brain usan `unicode61 remove_diacritics 2` **sin porter** (el contenido estará en español) y `prefix='2 3'`; las memorias tienen además una tabla trigram.
- **`tool_calls` no tiene FK a `executions`**: el hub registra cada llamada al terminar, antes de que el servidor registre la ejecución. `record_execution` es un upsert por `execution_id`.
- **`approvals`**: `token` (128 bits aleatorios en hex) es la PK; `status` ∈ pending, approved, consumed. `consume` es un único `UPDATE … WHERE token, tool, args_hash, status IN (pending, approved) AND expires_at > now`; solo tiene éxito si afecta a 1 fila, así que un token se gasta una vez y un intento con otra herramienta o hash no lo quema. Índice `approvals_expires` para limpieza.
- Los `CHECK` fijan los enumerados y `json_valid(...)` en las columnas JSON.

## Búsqueda de herramientas (`cerebro_search`)

1. La consulta se parte en términos (no alfanuméricos y camelCase, en minúsculas). Nunca se pasa texto del usuario a `MATCH` sin comillas.
2. `tools_fts`: términos entre comillas unidos con OR; orden por `bm25(tools_fts, 10, 5, 2, 3)` (name, title, description, search_text).
3. `tools_trgm`: trigramas de cada término de 3+ caracteres, unidos con OR (un `MATCH` literal de `isue` no casa con `issue`; sus trigramas `sue` sí).
4. RRF con k=60 sobre ambos rankings; candidatos = `max(limit*4, 50)`; desempate por nombre cualificado.
5. `limit` entre 1 y 25 (por defecto 8); consulta vacía → primeras N por nombre.
