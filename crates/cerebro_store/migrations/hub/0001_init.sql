-- hub.db v1: catálogo de servidores y herramientas downstream.
-- Se aplica sobre una conexión propia a hub.db (esquema `main` de esa conexión) y luego
-- se adjunta como `hub` a la conexión de brain.db. Los triggers y las FTS solo
-- referencian objetos de este mismo esquema.

CREATE TABLE servers (
    name             TEXT PRIMARY KEY NOT NULL,
    ts_namespace     TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'unknown'
                     CHECK (status IN ('unknown', 'ok', 'error', 'disabled')),
    last_error       TEXT,
    protocol_version TEXT,
    instructions     TEXT,
    catalog_hash     TEXT,
    updated_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE tools (
    id             INTEGER PRIMARY KEY,
    server         TEXT NOT NULL REFERENCES servers (name) ON DELETE CASCADE ON UPDATE CASCADE,
    name           TEXT NOT NULL,
    ts_name        TEXT NOT NULL,
    title          TEXT,
    description    TEXT NOT NULL DEFAULT '',
    input_schema   TEXT NOT NULL CHECK (json_valid(input_schema)),
    output_schema  TEXT CHECK (output_schema IS NULL OR json_valid(output_schema)),
    annotations    TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(annotations)),
    ts_signature   TEXT,
    ts_declaration TEXT,
    -- Texto derivado para búsqueda: servidor, nombre y ts_name partidos (snake/camel), alias.
    search_text    TEXT NOT NULL DEFAULT '',
    approx_tokens  INTEGER NOT NULL DEFAULT 0 CHECK (approx_tokens >= 0),
    schema_hash    TEXT NOT NULL,
    aliases        TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(aliases) AND json_type(aliases) = 'array'),
    available      INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1)),
    taint_labels   TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(taint_labels) AND json_type(taint_labels) = 'array'),
    updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (server, name)
) STRICT;

-- Búsqueda por ts_name (get_function_details / resolución de callbacks).
CREATE INDEX tools_server_ts_name ON tools (server, ts_name);

-- BM25 con stemming inglés (las descripciones MCP suelen estar en inglés).
CREATE VIRTUAL TABLE tools_fts USING fts5 (
    name, title, description, search_text,
    content = 'tools', content_rowid = 'id',
    tokenize = 'porter unicode61 remove_diacritics 2'
);

-- Coincidencias difusas (typos, subcadenas).
CREATE VIRTUAL TABLE tools_trgm USING fts5 (
    name, title, description, search_text,
    content = 'tools', content_rowid = 'id',
    tokenize = 'trigram'
);

CREATE TRIGGER tools_ai AFTER INSERT ON tools BEGIN
    INSERT INTO tools_fts (rowid, name, title, description, search_text)
    VALUES (new.id, new.name, new.title, new.description, new.search_text);
    INSERT INTO tools_trgm (rowid, name, title, description, search_text)
    VALUES (new.id, new.name, new.title, new.description, new.search_text);
END;

CREATE TRIGGER tools_ad AFTER DELETE ON tools BEGIN
    INSERT INTO tools_fts (tools_fts, rowid, name, title, description, search_text)
    VALUES ('delete', old.id, old.name, old.title, old.description, old.search_text);
    INSERT INTO tools_trgm (tools_trgm, rowid, name, title, description, search_text)
    VALUES ('delete', old.id, old.name, old.title, old.description, old.search_text);
END;

CREATE TRIGGER tools_au AFTER UPDATE ON tools BEGIN
    INSERT INTO tools_fts (tools_fts, rowid, name, title, description, search_text)
    VALUES ('delete', old.id, old.name, old.title, old.description, old.search_text);
    INSERT INTO tools_trgm (tools_trgm, rowid, name, title, description, search_text)
    VALUES ('delete', old.id, old.name, old.title, old.description, old.search_text);
    INSERT INTO tools_fts (rowid, name, title, description, search_text)
    VALUES (new.id, new.name, new.title, new.description, new.search_text);
    INSERT INTO tools_trgm (rowid, name, title, description, search_text)
    VALUES (new.id, new.name, new.title, new.description, new.search_text);
END;

-- Estadísticas de uso por herramienta. Clave natural (server, name) para que sobrevivan
-- a un replace_server_tools; se borran con el servidor.
CREATE TABLE tool_stats (
    server         TEXT NOT NULL REFERENCES servers (name) ON DELETE CASCADE ON UPDATE CASCADE,
    name           TEXT NOT NULL,
    calls          INTEGER NOT NULL DEFAULT 0 CHECK (calls >= 0),
    errors         INTEGER NOT NULL DEFAULT 0 CHECK (errors >= 0),
    total_ms       INTEGER NOT NULL DEFAULT 0 CHECK (total_ms >= 0),
    last_called_at TEXT,
    PRIMARY KEY (server, name)
) STRICT, WITHOUT ROWID;
