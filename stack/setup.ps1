# FORGEX Stack: instalación y configuración de un proyecto (Windows PowerShell).
# Uso: .\setup.ps1 <ruta-proyecto> [--clients=claude,antigravity,codex]
param(
  [Parameter(Position = 0)][string]$Project = (Get-Location).Path,
  [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest
)
$ErrorActionPreference = "Stop"
$StackDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# 1. Node ≥ 22.18 (lo exige GitNexus)
if (-not (Get-Command node -ErrorAction SilentlyContinue)) { Write-Error "Falta Node.js >= 22.18 (https://nodejs.org)"; exit 1 }
$nodeOk = node -e 'const [a,b]=process.versions.node.split(".").map(Number); console.log((a===22&&b>=18)||a>=24?"1":"0")'
if ($nodeOk -ne "1") { Write-Error "Node $(node -v) es demasiado antiguo: GitNexus exige 22.18+ (rama 22) o 24.11+"; exit 1 }

# 2. Herramientas globales (solo si faltan)
$pkgs = @{ "pctx" = "@portofcontext/pctx"; "gitnexus" = "gitnexus"; "mcp-server-memory" = "@modelcontextprotocol/server-memory" }
foreach ($bin in $pkgs.Keys) {
  if (-not (Get-Command $bin -ErrorAction SilentlyContinue)) { Write-Host "-> npm i -g $($pkgs[$bin])"; npm i -g $pkgs[$bin] }
}

# 3. Índice de GitNexus (sin sus skills ni su bloque AGENTS.md: aquí se usa detrás de pctx)
if (-not (Test-Path (Join-Path $Project ".git"))) { Write-Warning "$Project no es un repo git; GitNexus funciona mejor dentro de uno" }
Push-Location $Project
try { gitnexus analyze --skip-agents-md --skip-skills } finally { Pop-Location }

# 4. Configuración del proyecto y de los clientes (idempotente, con backups; usa `cmd /c` en Windows)
node (Join-Path $StackDir "configure.mjs") $Project @Rest

# 5. Dependencias del smoke test
Push-Location $StackDir
try { npm i --no-audit --no-fund --silent } finally { Pop-Location }
Write-Host "-> Verifica con: node `"$StackDir\verify.mjs`" `"$Project`""
