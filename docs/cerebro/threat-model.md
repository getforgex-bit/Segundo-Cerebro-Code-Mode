# Modelo de amenazas de Cerebro (Fase 0)

Estado: borrador de WS3 (2026-11-21). Método: STRIDE ligero por superficie. "Existe" = implementado y probado en la Fase 0; "Pendiente" = planificado.

## Activos

- Secretos y credenciales de servidores downstream (env de `.cerebro/config.json`, tokens de terceros).
- Datos del proyecto: código, el cerebro (`brain.db`), memorias, notas.
- Datos que devuelven las herramientas (pueden ser privados).
- Integridad de acciones externas (escrituras, borrados, envíos).
- Auditoría (`hub.db`) y aprobaciones.

## Actores

- **Usuario/cliente MCP legítimo**, cuya IA es **no confiable** (puede ser manipulada por inyección de prompts).
- **Contenido no confiable** que la IA lee (issues, páginas, resultados de herramientas).
- **Servidor downstream malicioso o comprometido** (descripciones, esquemas y resultados hostiles).
- **Otro usuario del equipo** (cerebro compartido en git; Fase 2).
- **Atacante remoto** (solo con el modo remoto, futuro).

## Superficies, amenazas y controles

### 1. Sandbox TS (código escrito por la IA)

| Amenaza | Control |
|---|---|
| Escape del sandbox / acceso a red o disco | Sandbox Deno de pctx, sin permisos de red ni FS salvo el puente de callbacks. **Existe** (heredado de pctx; no auditado por nosotros). |
| Consumo de CPU/memoria (DoS) | Timeouts de ejecución de pctx. `max_result_bytes` y `V8_MUTEX` global. **Parcial**: límites de memoria **Pendiente**. |
| Saltarse el hub llamando a un downstream directamente | Solo hay callbacks; no se usa `add_mcp`. **Existe** (`tests/sandbox.rs`). |

### 2. Puente sandbox ↔ hub

| Amenaza | Control |
|---|---|
| Suplantación de herramienta / id de callback | Los ids salen del catálogo (`server__tool`); el registry se reconstruye por ejecución. **Existe**. |
| Colisión de nombres TS que rompe o redirige llamadas | Deduplicación por (`ts_namespace`, `ts_name`) y rechazo de namespaces repetidos. **Existe** (tests). |
| Repetición de aprobaciones | Tokens de un solo uso ligados a `server.tool` + hash de argumentos y con TTL (600 s). **Existe** en el hub; la atomicidad real depende de `cerebro_store`. |
| Información sensible en errores | Errores con `CerebroErrorPayload` y mensajes acotados. Argumentos redactados en auditoría. **Existe** (heurística por nombre de clave). |

### 3. Servidores downstream

| Amenaza | Control |
|---|---|
| Procesos hijos que quedan vivos | `ChildWithCleanup`, `shutdown()` y reaper por inactividad. **Existe**. |
| Servidor que cuelga una llamada | `call_timeout` → `TIMEOUT`. **Existe**. |
| Servidor comprometido con anotaciones falsas (`readOnlyHint`) | Overrides de config prevalecen; ante conflicto de anotaciones gana `Confirm`. `default_unknown=confirm`. **Parcial**: un servidor que miente con `readOnlyHint=true` en una herramienta que escribe se ejecuta sin confirmar salvo override. Mitigación recomendada: `default_unknown`/override `confirm` para servidores no confiables. |
| Descripciones/esquemas con inyección de prompts (tool poisoning) | `schema_hash` por herramienta permite detectar cambios entre sincronizaciones. **Pendiente**: alertar/pedir reaprobación cuando cambie el hash. |
| Salida hostil que intenta manipular a la IA | El resultado intermedio se queda en el sandbox salvo lo que el código devuelva. **Parcial**; ver taint (M5). |
| Robo de secretos del entorno | El proceso hijo hereda el entorno del servidor Cerebro más `env` de la config. **Pendiente**: lista blanca de variables heredadas. |
| Comando arbitrario vía config | La config la escribe el usuario; no la controla la IA. **Pendiente**: aviso al cargar configs de repos clonados (`.cerebro/config.json` de un tercero ejecuta comandos). |

### 4. Aprobaciones

| Amenaza | Control |
|---|---|
| La IA se autoaprueba | El token solo se emite con `NEEDS_APPROVAL`; para que valga debe haber sido aprobado por el usuario fuera del sandbox (flujo del cliente o CLI). **Parcial**: en la Fase 0 `consume` acepta tokens **pendientes** (contrato `cerebro_types`), por lo que un cliente que reintente automáticamente con el token recibido aprobaría sin intervención humana. Es una decisión de WS4/coordinador que el cliente muestre la confirmación; recomendable exigir `approve()` previo (**Pendiente**, consulta al coordinador). |
| Reutilización con otros argumentos | Atado a `args_hash` (JSON canónico). **Existe** (test). |

### 5. Skills, índices y cerebro compartido (Fase 2)

| Amenaza | Control |
|---|---|
| Skills o memorias con instrucciones maliciosas en git | **Pendiente**: tratarlas como contenido no confiable; revisión humana; etiquetas `untrusted-input`. |
| Envenenamiento del índice de búsqueda | **Pendiente**: procedencia y `visibility` por memoria. |
| Fuga de datos privados en el cerebro compartido | **Pendiente**: campo `visibility` y filtros de sincronización. |

### 6. Auditoría y almacenamiento

| Amenaza | Control |
|---|---|
| Repudio de acciones | Registro por llamada (decisión, estado, hash, duración). **Existe** (hub); persistencia en `cerebro_store`. |
| Secretos en la auditoría | Redacción por nombre de clave (token, password, secret, key). **Existe**; no detecta secretos con nombre neutro (p. ej. `authorization`, `bearer`, `credential`). **Pendiente** ampliar la lista o detectar por valor. |
| Manipulación de `hub.db` | Permisos de archivo del usuario. **Pendiente** de evaluar cadena de hashes. |

### 7. Modo remoto (futuro)

Fuera de alcance de la Fase 0. Requerirá autenticación (OAuth), TLS, aislamiento por usuario, límites de tasa y revisión completa de esta tabla. **Pendiente**.

## Política de flujo M5 (taint, Fase 2)

Objetivo: mitigar la exfiltración por inyección de prompts. Cada herramienta lleva `taint_labels`: `private-data`, `untrusted-input`, `external-write` (por anotaciones y config). El hub acumula en `CallCtx.taint_seen` las etiquetas vistas en la ejecución. Si una misma ejecución combina las **tres**, la llamada `external-write` pasa a `Confirm` aunque la política base la permita. Solo Cerebro puede aplicarlo porque todas las llamadas pasan por el puente. Hoy el hub transporta `taint_seen` y `Policy::decide` lo recibe pero **no lo usa**; queda para la Fase 2.

## Riesgos residuales de la Fase 0

1. Confianza en anotaciones MCP de servidores no confiables.
2. Aprobación de tokens pendientes sin gesto explícito del usuario (ver §4).
3. Entorno heredado por los procesos hijos.
4. Sin detección de cambios en descripciones/esquemas entre sincronizaciones.
