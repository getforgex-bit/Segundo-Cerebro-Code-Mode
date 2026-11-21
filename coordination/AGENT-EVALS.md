# Evaluación empírica de modelos y agentes

> Los benchmarks públicos orientan, pero **no deciden**: decide la evidencia del propio proyecto. Este documento define cómo medimos y guarda el registro.
> Lo mantiene Opus. Neon y los coordinadores añaden filas desde los comentarios de cierre de cada Issue (PROTOCOL §8, regla 7).

## 1. Procedimiento ligero (por defecto)

Se reutiliza el trabajo real, **sin benchmarks costosos**. Al cerrar cada Issue se anota:

| Campo | Cómo se mide |
|---|---|
| Modelo + harness | p. ej. `Sonnet 5.5 · Claude Code cloud`, `Hydrogen · Antigravity swarm` |
| Éxito verificable | CI en verde y criterios de aceptación cumplidos (sí o no) |
| Rondas de revisión | Número de iteraciones hasta el merge |
| Defectos posteriores | Bugs o regresiones atribuidos después (revisión cruzada o CI) |
| Tiempo | Desde el arranque hasta `[LISTO]` o el PR listo |
| Coste o cuota | Claude: `cost_usd` de `get_session`. Antigravity: % de cuota antes y después (UI). |
| Supervisión | Consultas o intervenciones humanas o del coordinador necesarias |

**Métrica principal: coste por tarea completada correctamente** (no el precio por token).

## 2. Comparación directa (solo si hay motivo)

Solo para categorías **disputadas**, donde los benchmarks públicos no deciden: coordinación multiagente, arquitectura, memoria/RAG y optimización de tokens. También si un equipo rinde claramente por debajo de lo esperado.

- Una tarea pequeña y representativa, con **los mismos criterios de aceptación**, ejecutada en los dos harnesses.
- Se anota la diferencia de harness o herramientas como un factor, no como diferencia del modelo.
- Una reasignación estable de responsabilidades necesita evidencia de **≥ 3 tareas** y un ADR. No se reorganiza por diferencias marginales.

## 3. Registro

### Fase 0 (solo Equipo A; coste equivalente a API según `get_session`)

| Trabajo | Modelo · harness | Éxito | Coste | Notas |
|---|---|---|---|---|
| Coordinación de la Fase 0 (≈4,5 h) | Opus 5.5 `xhigh` · Claude Code | sí | $17,77 | Unos $11,6 son relecturas de caché de un contexto de 563k → **rotar al coordinador** |
| WS2: store + búsqueda + threat-model + permisos | Sonnet 5.5 · Claude Code | sí (16 + 11 tests) | $2,56 | 3 hitos extra asignados |
| WS3: política + hub | Sonnet 5.5 · Claude Code | sí (9 + 19 tests) | $2,10 | Una ronda extra por cruce con un cambio de contrato |
| WS4: servidor + E2E + docs (parcial) | Sonnet 5.5 · Claude Code | parcial (hub mock) | $2,07 | Se completa en el cierre de la Fase 0 |
| WS1: parche pctx + docs + CI | Sonnet 5.5 · Claude Code | sí (19 tests) | $1,53 | **Alcanzó el límite de 5 h** (5 sesiones en paralelo) |
| 4 workers de la primera tanda (~6 min cada uno) | Opus 5.5 · Claude Code | parcial (relevo) | $1,44–$2,51 cada uno | Sustituidos por Sonnet a petición del usuario |

**Conclusiones provisionales:**
- Sonnet es muy eficiente en briefs bien delimitados.
- El coste dominante es el contexto del coordinador.
- Concurrencia de A: máximo 1 Opus + 2–3 Sonnet.
