# AGENTS.md: FORGEX Stack (este repositorio)

> Idioma del proyecto: **español** (los identificadores de código, en inglés).

## Qué hay aquí

- **`stack/`: lo que se usa hoy.** Kit para dejar cualquier proyecto listo para agentes IA:
  - *Code Mode* con [pctx](https://github.com/portofcontext/pctx);
  - grafo de código con GitNexus;
  - memoria con `@modelcontextprotocol/server-memory`.

  Instalación y uso en [`stack/README.md`](stack/README.md).
- **Cerebro (congelado):** el gateway propio en Rust (`crates/cerebro*`), la coordinación de dos equipos (`coordination/`) y los ADR de `docs/cerebro/` están **pausados** desde el 2026-11-22. El punto de congelación es el commit `b122858` de `develop`, con la Fase 0 cerrada y la CI en verde. No se desarrolla salvo que el usuario lo reactive.
- `crates/pctx*`, `README.pctx.md` y `docs/*.md` son el fork de pctx (MIT); se dejan como upstream.

## Reglas

1. Rama de trabajo: `develop`. `main` solo con aprobación del usuario. Nada de `--force` ni de reescribir historia publicada. No tocar ramas `astra*`.
2. No reactivar Cerebro, los Issues cerrados ni los equipos de agentes sin una petición explícita del usuario.
3. No hacer push al remoto `upstream` (pctx) ni abrir PRs o issues allí sin autorización.
4. Verdad: no inventes resultados. Distingue *propuesta*, *hecho* y *verificado*, y cita el comando ejecutado.
5. GitNexus tiene licencia **PolyForm Noncommercial**. Antes de cualquier uso comercial, migrar (ver `stack/README.md`).
