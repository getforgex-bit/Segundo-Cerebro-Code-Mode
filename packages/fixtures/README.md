# @cerebro/fixtures

Servidores MCP stdio de prueba (CONTRACTS.md §5). Propietario: WS3. Consumidores: tests de `cerebro_hub` y `packages/e2e` (WS4).

| Fixture | Herramientas | Anotaciones |
|---|---|---|
| `dist/echo.js` | `echo({text})` → `{text}`; `add({a,b})` → `{sum}` | `readOnlyHint: true` |
| `dist/destructive.js` | `read_item({id})` → `{id,name,exists}`; `delete_item({id})` → texto JSON `{id,deleted}` (sin `outputSchema`; `isError` si el id no existe) | `readOnlyHint` / `destructiveHint` |

`destructive` guarda el estado en memoria por proceso (ids `1`, `2`, `3`).

```bash
cd packages && pnpm install && pnpm -r build   # → packages/fixtures/dist/*.js
pnpm --filter @cerebro/fixtures test           # smoke test por stdio
node packages/fixtures/dist/echo.js            # servidor MCP por stdio
```

`dist/` no se versiona (está en `.gitignore`): los tests Rust que lo necesitan se marcan `#[ignore]` si no existe.
