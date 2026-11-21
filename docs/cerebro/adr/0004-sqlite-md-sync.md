# ADR-0004: SQLite como fuente local y Markdown en git para el equipo

- **Estado:** propuesta (2026-11-21) · **Dueño:** WS2 · **Implementación:** Fase 2 (el esquema ya existe en la migración v1)

## Contexto

El cerebro del proyecto (memorias, decisiones, glosario) debe:
- ser rápido de consultar por la IA (búsqueda léxica, grafo);
- compartirse entre personas del equipo sin un servidor central;
- poder revisarse como cualquier otro cambio del repositorio.

SQLite es ideal para consultar pero opaco para git: un binario no se compara ni se combina.

## Decisión

- **SQLite (`brain.db`) es la fuente local de consulta**; no se versiona (`.cerebro/brain.db*`, `.cerebro/cache/` y `.cerebro/workspace/` van en `.gitignore`).
- **Markdown en `.cerebro/brain/<kind>/<slug>--<uid8>.md` es el formato de intercambio versionado**: un archivo por memoria, con frontmatter YAML (uid, kind, status, importance, tags, links, author, created/updated) y el cuerpo en Markdown. Un archivo por memoria hace que git combine por archivo.
- **Visibilidad por memoria**: `team` (por defecto) o `personal`; las personales nunca se exportan.
- **Qué no se exporta**: el índice de código ni los chunks (se reconstruyen). Sí se exportan los enlaces memoria→código.

### Sincronización (`cerebro brain sync`, idempotente)

Columnas de apoyo en `memories`: `sync_hash` (hash del archivo en el último sync) y `synced_at`.

1. **Importa** los archivos cuyo hash difiere del `sync_hash`.
2. **Exporta** lo cambiado en la BD desde la última sincronización.
3. Si cambiaron **ambos lados**, escribe `<archivo>.conflict.md` y marca `status = conflict`. Nada se sobrescribe en silencio.
4. En `serve` hay un watcher sobre `.cerebro/brain/` y un hook opcional `post-merge` de git.

La identidad estable es el `uid` (ULID), no la ruta: renombrar un archivo no duplica la memoria.

## Alternativas descartadas

- **Versionar `brain.db`**: binario, sin diff ni merge.
- **Servidor central o SQLite replicado**: infraestructura y credenciales que el equipo no quiere mantener.
- **Solo Markdown, sin SQLite**: sin FTS, sin grafo ni consultas rápidas.
- **CRDT**: complejidad desproporcionada para memorias de baja frecuencia de escritura.

## Consecuencias

- Los conflictos son explícitos y los resuelve una persona; coste: algo de fricción en equipos que editan la misma memoria.
- El formato Markdown queda como contrato público y debe versionarse (campo de versión en el frontmatter cuando se implemente).
- Los secretos no deben llegar a las memorias exportables: el extractor y `cerebro_remember` han de redactar antes de exportar (riesgo a cubrir en la implementación de la Fase 2).
- Riesgo conocido: relojes distintos entre máquinas; por eso la detección usa hashes y no marcas de tiempo.
