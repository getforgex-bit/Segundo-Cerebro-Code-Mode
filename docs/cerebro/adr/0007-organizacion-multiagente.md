# ADR-0007: Organización multiagente en dos equipos (Claude Code + Antigravity)

- **Estado:** aceptada (2026-11-21) · **Dueño:** Opus (coordinador del Equipo A) · **Revisable** al cierre de cada fase

## Contexto

FORGEX Brain lo desarrollan dos equipos de agentes:
- **Equipo A**, en Claude Code: Opus 5.5 coordina y los trabajadores son Sonnet 5.5.
- **Equipo B**, en Google Antigravity: Gemini 4 Neon coordina y los trabajadores son Gemini 4 Hydrogen.

Hasta ahora solo existía el sistema de coordinación de la Fase 0 (`coordination/`, mensajes `send_message` entre sesiones Claude), que funcionó: 4 relevos sin pérdida y 5 líneas integradas. No hay canal directo entre Claude Code y Antigravity.

## Decisión

1. **Reparto por la frontera real del código:**
   - A: gateway/Code Mode, ya construido, e integración.
   - B: subsistema cerebro, indexado y grafo, recuperación, integraciones de cliente y evals. Es greenfield detrás del trait `Brain`.

   Detalle y propiedad por rutas: `coordination/TEAMS.md`.
2. **Jerarquía híbrida:**
   - el usuario decide el producto;
   - Opus es el arquitecto principal e integrador (único que hace merge a `develop`; desempata solo en integración);
   - Neon es coarquitecto con autoridad plena en sus rutas e igual a Opus en investigación;
   - los contratos compartidos requieren el visto bueno de ambos.
3. **Canales:**
   - dentro de A, `send_message` (PROTOCOL §1–7);
   - dentro de B, el tablero del swarm;
   - **entre equipos, GitHub Issues y PRs** (PROTOCOL §8);
   - decisiones duraderas en ADR y CONTRACTS.
4. **Ramas:** troncal `develop`, `main` estable, ramas de trabajo `a/*` y `b/*` (PROTOCOL §10).
5. **Revisión cruzada entre familias de modelos**, obligatoria en cambios críticos.
6. **Recursos:**
   - concurrencia limitada (A: 1 Opus + 2–3 Sonnet; B: Neon + 3–4 Hydrogen + verificador);
   - rotación de coordinadores (~300k y ~250k tokens);
   - sin pagos extra salvo autorización.

## Evidencia

- **Benchmarks públicos** (aportados por el usuario; orientativos):
  - Terminal-Bench y FrontierCode muestran una ventaja pequeña de Claude.
  - CursorBench, ALE, GraphWalks (+28 pts), SWE-sweep y AutomationBench muestran ventaja de Gemini en trabajo ambiguo, multiarchivo, relacional, de largo horizonte y de automatización.
  - Ninguno mide coordinación ni memoria/RAG: eso se mide en `coordination/AGENT-EVALS.md`.
- **Entorno:** B corre en la máquina del usuario, donde están los clientes reales (Claude Desktop, Codex, Antigravity). A corre en la nube, sin ellos. Por eso B se encarga de `cerebro_clients`.
- **Coste medido en la Fase 0:**
  - Sonnet completó briefs enteros por $1,5–2,6.
  - Opus como trabajador rindió peor por dólar.
  - El contexto del coordinador supuso ~51 % del gasto.
  - Con 5 sesiones en paralelo se alcanzó el límite de 5 h.

## Arranque

El Equipo B empieza cuando se cumplen estas condiciones, que Neon puede verificar sin esperar respuesta de Opus:
1. `develop` existe con la CI en verde.
2. `AGENTS.md`, `coordination/TEAMS.md` y PROTOCOL §8–§10 están en `develop`.
3. Existen el Issue "Coordinación A↔B" y los Issues B1–B5 marcados `status:ready`.

A partir de ese momento, A y B trabajan en paralelo en rutas disjuntas. Los únicos puntos de contacto son los contratos `Brain` (B1) y la API de `cerebro_clients` (B4).

## Alternativas descartadas

- **Reparto fijo sin evidencia** (memoria e investigación solo para B): infrautiliza a Neon en programación compleja.
- **Ocho agentes siempre activos:** la cuota se agota y no hay paralelismo útil.
- **Un sistema de documentación paralelo:** se amplía `coordination/`, no se sustituye.

## Consecuencias

- Hay que mantener la disciplina de Issues: firma y anti-ruido.
- Opus revisa Issues y PRs en check-ins, porque no existe un canal en tiempo real.
- La coordinación Claude↔Antigravity **solo se considera verificada** cuando Neon publique su acuse en el Issue raíz.
