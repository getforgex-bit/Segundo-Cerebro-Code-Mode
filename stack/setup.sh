#!/usr/bin/env bash
# FORGEX Stack: instalación y configuración de un proyecto (macOS/Linux).
# Uso: ./setup.sh <ruta-proyecto> [--clients=claude,antigravity,codex]
set -euo pipefail

STACK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="${1:-$PWD}"
shift || true

# 1. Node ≥ 22.18 (lo exige GitNexus)
if ! command -v node >/dev/null 2>&1; then echo "✘ Falta Node.js ≥ 22.18 (https://nodejs.org)"; exit 1; fi
NODE_OK=$(node -e 'const [a,b]=process.versions.node.split(".").map(Number); console.log((a===22&&b>=18)||a>=24?"1":"0")')
if [ "$NODE_OK" != "1" ]; then echo "✘ Node $(node -v) es demasiado antiguo: GitNexus exige 22.18+ (rama 22) o 24.11+"; exit 1; fi

# 2. Herramientas globales (solo si faltan)
for pkg in "pctx:@portofcontext/pctx" "gitnexus:gitnexus" "mcp-server-memory:@modelcontextprotocol/server-memory"; do
  bin="${pkg%%:*}"; npmpkg="${pkg#*:}"
  if ! command -v "$bin" >/dev/null 2>&1; then echo "→ npm i -g $npmpkg"; npm i -g "$npmpkg"; fi
done

# 3. Índice de GitNexus. Sin las skills ni el bloque AGENTS.md propios de GitNexus:
#    aquí GitNexus se usa detrás de pctx y las instrucciones vienen de FORGEX Stack.
if [ ! -d "$PROJECT/.git" ]; then echo "⚠ $PROJECT no es un repo git; GitNexus funciona mejor dentro de uno"; fi
(cd "$PROJECT" && gitnexus analyze --skip-agents-md --skip-skills)

# 4. Configuración del proyecto y de los clientes (idempotente, con backups)
node "$STACK_DIR/configure.mjs" "$PROJECT" "$@"

# 5. Dependencias del smoke test
(cd "$STACK_DIR" && npm i --no-audit --no-fund --silent)
echo "→ Verifica con: node \"$STACK_DIR/verify.mjs\" \"$PROJECT\""
