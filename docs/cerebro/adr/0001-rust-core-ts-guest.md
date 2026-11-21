# ADR-0001: Núcleo en Rust, TypeScript como lenguaje invitado, SQLite como estado

- **Estado:** aceptada (2026-11-21) · **Dueño:** coordinador

## Contexto

Cerebro debe:
- exponer a cualquier cliente MCP un gateway estilo *Code Mode* (la IA escribe código que llama herramientas en un sandbox);
- sostener un cerebro de proyecto persistente.

El stack pedido es Rust, TypeScript y SQLite.

## Decisión

- **Rust** para todo lo que corre en el binario: servidor MCP, hub downstream, política, store, búsqueda, indexado y CLI. El motor de ejecución sale del fork de pctx (ver ADR-0002 y ADR-0003).
- **TypeScript** es el lenguaje que **escribe la IA** dentro del sandbox, el de los `.d.ts` que lee, el de los fixtures y E2E (SDK MCP oficial) y el de los evals.
- **SQLite** guarda el estado local: catálogo, auditoría, aprobaciones, memorias y grafo. El equipo comparte el cerebro vía Markdown en git (ADR-0004).

## Alternativas descartadas

- Orquestador Node con addon nativo: dos runtimes que instalar y más superficie.
- Motor propio QuickJS: lo cubre pctx; ver ADR-0003.

## Consecuencias

- Un binario por plataforma; el tiempo de compilación sube por V8.
- Los contratos entre crates viven en `crates/cerebro_types`.
