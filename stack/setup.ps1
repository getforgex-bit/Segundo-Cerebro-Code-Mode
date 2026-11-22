# FORGEX Stack: instalación y configuración de un proyecto (Windows PowerShell 5.1+ y PowerShell 7).
# Uso (la política de ejecución por defecto de Windows bloquea los .ps1; -ExecutionPolicy Bypass solo afecta a esta ejecución):
#   powershell -ExecutionPolicy Bypass -File .\stack\setup.ps1 C:\ruta\a\tu\proyecto
#   powershell -ExecutionPolicy Bypass -File .\stack\setup.ps1 C:\ruta\a\tu\proyecto -Clients claude,codex
param(
  [Parameter(Position = 0)][string]$Project = (Get-Location).Path,
  [string[]]$Clients = @("claude", "antigravity", "codex"),
  [switch]$SkipVerify
)
$ErrorActionPreference = "Stop"
$StackDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# PowerShell 5.1 no se detiene cuando un programa externo falla: se comprueba $LASTEXITCODE.
function Invoke-Step([string]$Label, [scriptblock]$Block) {
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "Falló: $Label (código $LASTEXITCODE)" }
}

# 1. Node ^22.18 o >=24.11 (lo exige GitNexus). Se analiza aquí y no con `node -e`, porque
#    PowerShell 5.1 rompe las comillas dobles de los argumentos de programas externos.
if (-not (Get-Command node -ErrorAction SilentlyContinue)) { throw "Falta Node.js (^22.18 o >=24.11): instala la LTS de https://nodejs.org" }
$ver = (node -v).Trim().TrimStart("v").Split(".")
$major = [int]$ver[0]; $minor = [int]$ver[1]
if (-not (($major -eq 22 -and $minor -ge 18) -or ($major -eq 24 -and $minor -ge 11) -or $major -ge 25)) {
  throw "Node $(node -v) no vale: GitNexus exige ^22.18 o >=24.11 (instala la LTS de https://nodejs.org)"
}

# 2. Herramientas globales (solo si faltan)
$Project = (Resolve-Path $Project).Path
$pkgs = [ordered]@{ "pctx" = "@portofcontext/pctx"; "gitnexus" = "gitnexus"; "mcp-server-memory" = "@modelcontextprotocol/server-memory" }
foreach ($bin in $pkgs.Keys) {
  if (-not (Get-Command $bin -ErrorAction SilentlyContinue)) {
    $pkg = $pkgs[$bin]
    Write-Host "-> npm i -g $pkg"
    Invoke-Step "npm i -g $pkg" { npm i -g $pkg }
  }
}
foreach ($bin in $pkgs.Keys) {
  if (-not (Get-Command $bin -ErrorAction SilentlyContinue)) {
    throw "No encuentro '$bin' en el PATH tras instalarlo. Añade al PATH la carpeta que muestra 'npm prefix -g' y abre una terminal nueva."
  }
}

# 3. Índice de GitNexus (sin sus skills ni su bloque AGENTS.md: aquí se usa detrás de pctx)
if (-not (Test-Path (Join-Path $Project ".git"))) { Write-Warning "$Project no es un repo git; GitNexus funciona mejor dentro de uno" }
Push-Location $Project
try { Invoke-Step "gitnexus analyze" { gitnexus analyze --skip-agents-md --skip-skills } } finally { Pop-Location }

# 4. Configuración del proyecto y de los clientes (idempotente, con backups; en Windows lanza los CLI con `cmd /c`)
$clientsArg = "--clients=" + ($Clients -join ",")
Invoke-Step "configure.mjs" { node (Join-Path $StackDir "configure.mjs") $Project $clientsArg }

# 5. Smoke test: arranca pctx, usa la memoria y GitNexus a través de Code Mode y mide tokens
Push-Location $StackDir
try { Invoke-Step "npm i (stack)" { npm i --no-audit --no-fund --silent } } finally { Pop-Location }
if (-not $SkipVerify) {
  Write-Host ""
  Invoke-Step "verify.mjs" { node (Join-Path $StackDir "verify.mjs") $Project }
}
