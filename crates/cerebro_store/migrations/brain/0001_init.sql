-- brain.db v1: esquema completo de la Fase 2 (evita una segunda migración).
-- Se aplica sobre la conexión principal. Solo referencia objetos de este esquema.

-- ───────────── Memorias ─────────────
CREATE TABLE memories (
    id          INTEGER PRIMARY KEY,
    uid         TEXT NOT NULL UNIQUE,
    kind        TEXT NOT NULL CHECK (kind IN ('decision', 'convention', 'fact', 'note', 'task',
                                              'question', 'glossary', 'incident', 'preference', 'summary')),
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'superseded', 'archived', 'conflict')),
    importance  INTEGER NOT NULL DEFAULT 3 CHECK (importance BETWEEN 1 AND 5),
    tags        TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tags) AND json_type(tags) = 'array'),
    source      TEXT,
    session_id  TEXT,
    supersedes  TEXT,
    visibility  TEXT NOT NULL DEFAULT 'team' CHECK (visibility IN ('team', 'personal')),
    author      TEXT,
    sync_hash   TEXT,
    synced_at   TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX memories_kind_status ON memories (kind, status);
CREATE INDEX memories_sync ON memories (visibility, synced_at);

-- Español: sin stemming inglés (porter), con prefijos.
CREATE VIRTUAL TABLE memories_fts USING fts5 (
    title, body, tags,
    content = 'memories', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'
);
CREATE VIRTUAL TABLE memories_trgm USING fts5 (
    title, body, tags,
    content = 'memories', content_rowid = 'id',
    tokenize = 'trigram'
);
CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts (rowid, title, body, tags) VALUES (new.id, new.title, new.body, new.tags);
    INSERT INTO memories_trgm (rowid, title, body, tags) VALUES (new.id, new.title, new.body, new.tags);
END;
CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts (memories_fts, rowid, title, body, tags) VALUES ('delete', old.id, old.title, old.body, old.tags);
    INSERT INTO memories_trgm (memories_trgm, rowid, title, body, tags) VALUES ('delete', old.id, old.title, old.body, old.tags);
END;
CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts (memories_fts, rowid, title, body, tags) VALUES ('delete', old.id, old.title, old.body, old.tags);
    INSERT INTO memories_trgm (memories_trgm, rowid, title, body, tags) VALUES ('delete', old.id, old.title, old.body, old.tags);
    INSERT INTO memories_fts (rowid, title, body, tags) VALUES (new.id, new.title, new.body, new.tags);
    INSERT INTO memories_trgm (rowid, title, body, tags) VALUES (new.id, new.title, new.body, new.tags);
END;

-- ───────────── Índice de proyecto ─────────────
CREATE TABLE documents (
    id                INTEGER PRIMARY KEY,
    path              TEXT NOT NULL UNIQUE,
    kind              TEXT NOT NULL CHECK (kind IN ('markdown', 'code', 'config', 'data', 'notebook',
                                                    'pdf', 'office', 'text', 'other')),
    language          TEXT,
    hash              TEXT NOT NULL,
    size              INTEGER NOT NULL DEFAULT 0 CHECK (size >= 0),
    mtime             TEXT,
    extractor         TEXT,
    extractor_version INTEGER NOT NULL DEFAULT 0,
    indexed_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE symbols (
    id          INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    parent_id   INTEGER REFERENCES symbols (id) ON DELETE SET NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    signature   TEXT,
    doc         TEXT,
    start_line  INTEGER,
    end_line    INTEGER
) STRICT;
CREATE INDEX symbols_document ON symbols (document_id);
CREATE INDEX symbols_name ON symbols (name);
CREATE VIRTUAL TABLE symbols_fts USING fts5 (
    name, signature, doc,
    content = 'symbols', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'
);
CREATE TRIGGER symbols_ai AFTER INSERT ON symbols BEGIN
    INSERT INTO symbols_fts (rowid, name, signature, doc) VALUES (new.id, new.name, new.signature, new.doc);
END;
CREATE TRIGGER symbols_ad AFTER DELETE ON symbols BEGIN
    INSERT INTO symbols_fts (symbols_fts, rowid, name, signature, doc) VALUES ('delete', old.id, old.name, old.signature, old.doc);
END;
CREATE TRIGGER symbols_au AFTER UPDATE ON symbols BEGIN
    INSERT INTO symbols_fts (symbols_fts, rowid, name, signature, doc) VALUES ('delete', old.id, old.name, old.signature, old.doc);
    INSERT INTO symbols_fts (rowid, name, signature, doc) VALUES (new.id, new.name, new.signature, new.doc);
END;

CREATE TABLE chunks (
    id          INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    symbol_id   INTEGER REFERENCES symbols (id) ON DELETE SET NULL,
    ord         INTEGER NOT NULL DEFAULT 0,
    heading     TEXT,
    content     TEXT NOT NULL,
    start_line  INTEGER,
    end_line    INTEGER,
    hash        TEXT
) STRICT;
CREATE INDEX chunks_document ON chunks (document_id, ord);
CREATE INDEX chunks_symbol ON chunks (symbol_id);
CREATE VIRTUAL TABLE chunks_fts USING fts5 (
    heading, content,
    content = 'chunks', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'
);
CREATE TRIGGER chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts (rowid, heading, content) VALUES (new.id, new.heading, new.content);
END;
CREATE TRIGGER chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts (chunks_fts, rowid, heading, content) VALUES ('delete', old.id, old.heading, old.content);
END;
CREATE TRIGGER chunks_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts (chunks_fts, rowid, heading, content) VALUES ('delete', old.id, old.heading, old.content);
    INSERT INTO chunks_fts (rowid, heading, content) VALUES (new.id, new.heading, new.content);
END;

-- Grafo con refs URI: memory:, file:, symbol:, skill:, tool:, commit:
CREATE TABLE links (
    id         INTEGER PRIMARY KEY,
    src_uri    TEXT NOT NULL,
    dst_uri    TEXT NOT NULL,
    rel        TEXT NOT NULL CHECK (rel IN ('relates_to', 'supersedes', 'depends_on', 'implements',
                                            'mentions', 'imports', 'defines', 'tests', 'documents',
                                            'blocks', 'derived_from', 'changed')),
    origin     TEXT NOT NULL DEFAULT 'auto' CHECK (origin IN ('auto', 'user', 'agent')),
    confidence REAL NOT NULL DEFAULT 1.0 CHECK (confidence BETWEEN 0.0 AND 1.0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (src_uri, dst_uri, rel)
) STRICT;
CREATE INDEX links_dst ON links (dst_uri, rel);

CREATE TABLE skills (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    body        TEXT NOT NULL DEFAULT '',
    path        TEXT,
    hash        TEXT,
    trusted     INTEGER NOT NULL DEFAULT 0 CHECK (trusted IN (0, 1)),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE VIRTUAL TABLE skills_fts USING fts5 (
    name, description, body,
    content = 'skills', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'
);
CREATE TRIGGER skills_ai AFTER INSERT ON skills BEGIN
    INSERT INTO skills_fts (rowid, name, description, body) VALUES (new.id, new.name, new.description, new.body);
END;
CREATE TRIGGER skills_ad AFTER DELETE ON skills BEGIN
    INSERT INTO skills_fts (skills_fts, rowid, name, description, body) VALUES ('delete', old.id, old.name, old.description, old.body);
END;
CREATE TRIGGER skills_au AFTER UPDATE ON skills BEGIN
    INSERT INTO skills_fts (skills_fts, rowid, name, description, body) VALUES ('delete', old.id, old.name, old.description, old.body);
    INSERT INTO skills_fts (rowid, name, description, body) VALUES (new.id, new.name, new.description, new.body);
END;

-- ───────────── Sesiones, ejecuciones y auditoría ─────────────
CREATE TABLE sessions (
    id         TEXT PRIMARY KEY NOT NULL,
    client     TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ended_at   TEXT
) STRICT;

CREATE TABLE executions (
    execution_id        TEXT PRIMARY KEY NOT NULL,
    session_id          TEXT,
    code_hash           TEXT NOT NULL,
    status              TEXT NOT NULL,
    error               TEXT,
    tool_calls          INTEGER NOT NULL DEFAULT 0 CHECK (tool_calls >= 0),
    raw_bytes_processed INTEGER NOT NULL DEFAULT 0 CHECK (raw_bytes_processed >= 0),
    result_bytes        INTEGER NOT NULL DEFAULT 0 CHECK (result_bytes >= 0),
    duration_ms         INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

-- Sin FK a executions: el hub registra las llamadas antes que la ejecución.
CREATE TABLE tool_calls (
    id            INTEGER PRIMARY KEY,
    execution_id  TEXT NOT NULL,
    server        TEXT NOT NULL,
    tool          TEXT NOT NULL,
    args_redacted TEXT NOT NULL DEFAULT 'null' CHECK (json_valid(args_redacted)),
    args_hash     TEXT NOT NULL,
    decision      TEXT NOT NULL CHECK (decision IN ('allowed', 'confirmed', 'denied', 'needs_approval')),
    status        TEXT NOT NULL CHECK (status IN ('ok', 'error', 'timeout', 'not_run')),
    error         TEXT,
    result_bytes  INTEGER NOT NULL DEFAULT 0 CHECK (result_bytes >= 0),
    duration_ms   INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX tool_calls_execution ON tool_calls (execution_id);
CREATE INDEX tool_calls_tool ON tool_calls (server, tool);

CREATE TABLE approvals (
    token        TEXT PRIMARY KEY NOT NULL,
    execution_id TEXT NOT NULL,
    tool         TEXT NOT NULL,
    args_hash    TEXT NOT NULL,
    reason       TEXT NOT NULL DEFAULT '',
    status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'consumed')),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at   TEXT NOT NULL,
    approved_at  TEXT,
    consumed_at  TEXT
) STRICT, WITHOUT ROWID;
CREATE INDEX approvals_expires ON approvals (expires_at);

CREATE TABLE workspace_kv (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL CHECK (json_valid(value)),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT, WITHOUT ROWID;

-- Resultados grandes que se quedan fuera del contexto del modelo (M6).
CREATE TABLE result_handles (
    handle       TEXT PRIMARY KEY NOT NULL,
    execution_id TEXT,
    content      TEXT NOT NULL,
    bytes        INTEGER NOT NULL DEFAULT 0 CHECK (bytes >= 0),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at   TEXT
) STRICT, WITHOUT ROWID;

CREATE TABLE sync_state (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT, WITHOUT ROWID;

-- Ahorro estimado por ejecución (≈ 4 bytes por token). Solo tablas de brain.
CREATE VIEW v_token_savings AS
SELECT execution_id,
       created_at,
       tool_calls,
       raw_bytes_processed,
       result_bytes,
       MAX(raw_bytes_processed - result_bytes, 0)     AS saved_bytes,
       MAX(raw_bytes_processed - result_bytes, 0) / 4 AS approx_tokens_saved
FROM executions;
