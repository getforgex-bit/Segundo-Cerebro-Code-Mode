## FORGEX Stack: cómo trabajar en este proyecto (agentes IA)

Tienes **un** servidor MCP, `forgex` (pctx en *Code Mode*), con 3 herramientas: `list_functions`, `get_function_details` y `execute_typescript`. Detrás están:
- **`Gitnexus`**: grafo de conocimiento del código (símbolos, llamadas, flujos, impacto).
- **`Memory`**: memoria persistente del proyecto (decisiones, convenciones, tareas), guardada en `.forgex/memory.jsonl`, que se versiona.

### Reglas para ahorrar tokens
1. **No llames a `list_functions` sin filtrar** (unos 12k tokens). Usa esta chuleta y pide solo lo que necesites con `get_function_details({ functions: ["Gitnexus.impact"] })`.
2. Encadena las llamadas en **un solo** `execute_typescript` y devuelve **solo** el resumen que necesitas, no los resultados crudos.
3. El código es TypeScript con `async function run() { … return … }`. Se comprueban los tipos antes de ejecutar.
4. Si tienes terminal, la CLI de GitNexus no cuesta tokens de contexto: `gitnexus query "<concepto>"`, `gitnexus context <símbolo>`, `gitnexus impact <símbolo> --direction upstream`, `gitnexus detect-changes`.

### Chuleta (nombres reales)
- `Gitnexus.query({ search_query, limit?, repo? })`: flujos y símbolos relacionados con un concepto.
- `Gitnexus.context({ name | uid | file_path, repo? })`: vista de 360° de un símbolo (quién lo llama, a quién llama, procesos).
- `Gitnexus.impact({ target, direction: "upstream" | "downstream", maxDepth?, repo? })`: qué se rompe si cambias algo.
- `Gitnexus.detectChanges({ scope?, base_ref?, repo? })`: mapea el diff de git a símbolos y flujos afectados.
- `Gitnexus.trace`, `Gitnexus.cypher` y `Gitnexus.listRepos` también existen (consulta su firma con `get_function_details`).
- `Memory.searchNodes({ query })`, `Memory.openNodes({ names })` y `Memory.readGraph()`: consultar.
- `Memory.createEntities({ entities: [{ name, entityType, observations }] })`, `Memory.addObservations({ observations: [{ entityName, contents }] })` y `Memory.createRelations({ relations: [{ from, to, relationType }] })`: guardar.

### Flujo de trabajo
1. **Al empezar:** `Memory.searchNodes` con el tema de la tarea, para recuperar decisiones y convenciones previas.
2. **Antes de editar:** `Gitnexus.context` y `Gitnexus.impact` sobre los símbolos que vas a tocar.
3. **Antes de hacer commit:** `gitnexus detect-changes` (o `Gitnexus.detectChanges`) para revisar el impacto real del diff.
4. **Al terminar:** guarda en `Memory` las decisiones, convenciones o descubrimientos que valga la pena recordar. Usa entidades con `entityType` `decision`, `convention`, `task` o `gotcha` y observaciones breves y verificables.
5. Si el índice está desactualizado, ejecuta `gitnexus analyze --skip-agents-md --skip-skills` (es incremental).
