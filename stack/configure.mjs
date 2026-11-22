// FORGEX Stack: configura un proyecto (multiplataforma). Lo llaman setup.sh / setup.ps1.
// Uso: node configure.mjs <ruta-proyecto> [--clients claude,antigravity,codex]
// Idempotente: solo añade o actualiza la entrada "forgex"; antes de modificar un archivo, guarda una copia .bak.
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const project = resolve(args.find((a) => !a.startsWith("--")) ?? process.cwd());
const clientsArg = args.find((a) => a.startsWith("--clients="))?.split("=")[1] ?? "claude,antigravity,codex";
const clients = new Set(clientsArg.split(",").map((s) => s.trim()));
const isWin = process.platform === "win32";
const stamp = new Date().toISOString().replace(/[:.]/g, "-");

// En Windows los CLI de npm son shims .cmd: hay que lanzarlos con `cmd /c`.
const cmd = (bin, binArgs = []) => (isWin ? { command: "cmd", args: ["/c", bin, ...binArgs] } : { command: bin, args: binArgs });

function backup(file) {
  if (existsSync(file)) copyFileSync(file, `${file}.bak-${stamp}`);
}
function writeIfChanged(file, content, label) {
  const prev = existsSync(file) ? readFileSync(file, "utf8") : null;
  if (prev === content) return console.log(`  = ${label} sin cambios`);
  mkdirSync(dirname(file), { recursive: true });
  backup(file);
  writeFileSync(file, content);
  console.log(`  ✔ ${label}${prev === null ? " (nuevo)" : " (actualizado; copia .bak)"}`);
}
function readJson(file) {
  if (!existsSync(file)) return {};
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch {
    throw new Error(`${file} no es JSON válido; corrígelo o muévelo antes de continuar`);
  }
}

if (!existsSync(project)) throw new Error(`No existe el proyecto ${project}`);
console.log(`FORGEX Stack · configurando ${project}`);

// 1. pctx.json: Code Mode sobre memoria + GitNexus.
const pctxPath = join(project, "pctx.json");
mkdirSync(join(project, ".forgex"), { recursive: true });
const template = JSON.parse(readFileSync(join(here, "templates", "pctx.json"), "utf8"));
const existing = readJson(pctxPath);
const servers = (existing.servers ?? []).filter((s) => !["memory", "gitnexus"].includes(s.name));
for (const s of template.servers) {
  const spawn = cmd(s.command, s.args ?? []);
  const entry = { name: s.name, ...spawn };
  if (s.env) {
    entry.env = Object.fromEntries(
      Object.entries(s.env).map(([k, v]) => [k, v === "__PROJECT__/.forgex/memory.jsonl" ? join(project, ".forgex", "memory.jsonl") : v]),
    );
  }
  servers.push(entry);
}
const pctxConfig = { ...template, ...existing, servers };
writeIfChanged(pctxPath, `${JSON.stringify(pctxConfig, null, 2)}\n`, "pctx.json");

// 2. Registro del servidor "forgex" (pctx por stdio) en los clientes, a nivel de proyecto.
const forgex = cmd("pctx", ["--config", pctxPath, "code-mode", "start", "--stdio", "--no-banner"]);
const mcpJsonEntry = { command: forgex.command, args: forgex.args };
function mergeMcpServers(file, label) {
  const cfg = readJson(file);
  cfg.mcpServers = { ...(cfg.mcpServers ?? {}), forgex: mcpJsonEntry };
  writeIfChanged(file, `${JSON.stringify(cfg, null, 2)}\n`, label);
}
if (clients.has("claude")) mergeMcpServers(join(project, ".mcp.json"), "Claude Code (.mcp.json)");
if (clients.has("antigravity")) mergeMcpServers(join(project, ".agents", "mcp_config.json"), "Antigravity (.agents/mcp_config.json)");
if (clients.has("codex")) {
  const file = join(project, ".codex", "config.toml");
  const prev = existsSync(file) ? readFileSync(file, "utf8") : "";
  const block = `[mcp_servers.forgex]\ncommand = ${JSON.stringify(forgex.command)}\nargs = ${JSON.stringify(forgex.args)}\n`;
  const next = /\[mcp_servers\.forgex\][\s\S]*?(?=\n\[|$)/.test(prev)
    ? prev.replace(/\[mcp_servers\.forgex\][\s\S]*?(?=\n\[|$)/, block.trimEnd())
    : `${prev}${prev && !prev.endsWith("\n") ? "\n" : ""}${prev ? "\n" : ""}${block}`;
  writeIfChanged(file, next.endsWith("\n") ? next : `${next}\n`, "Codex (.codex/config.toml)");
}

// 3. Instrucciones para los agentes: bloque entre marcadores en AGENTS.md y CLAUDE.md que importa @AGENTS.md.
const block = readFileSync(join(here, "templates", "AGENTS.forgex.md"), "utf8").trim();
const agentsPath = join(project, "AGENTS.md");
const agentsPrev = existsSync(agentsPath) ? readFileSync(agentsPath, "utf8") : "";
const marked = `<!-- forgex:start -->\n${block}\n<!-- forgex:end -->`;
const agentsNext = /<!-- forgex:start -->[\s\S]*<!-- forgex:end -->/.test(agentsPrev)
  ? agentsPrev.replace(/<!-- forgex:start -->[\s\S]*<!-- forgex:end -->/, marked)
  : `${agentsPrev}${agentsPrev ? "\n\n" : ""}${marked}\n`;
writeIfChanged(agentsPath, agentsNext.endsWith("\n") ? agentsNext : `${agentsNext}\n`, "AGENTS.md (bloque forgex)");
const claudePath = join(project, "CLAUDE.md");
const claudePrev = existsSync(claudePath) ? readFileSync(claudePath, "utf8") : "";
if (!/^@AGENTS\.md\s*$/m.test(claudePrev)) {
  writeIfChanged(claudePath, `@AGENTS.md\n${claudePrev ? `\n${claudePrev}` : ""}`, "CLAUDE.md (@AGENTS.md)");
}

// 4. .gitignore: el índice de GitNexus no se versiona; la memoria (.forgex/memory.jsonl) sí.
const giPath = join(project, ".gitignore");
const giPrev = existsSync(giPath) ? readFileSync(giPath, "utf8") : "";
if (!/^\.gitnexus\/?\s*$/m.test(giPrev)) {
  writeIfChanged(giPath, `${giPrev}${giPrev && !giPrev.endsWith("\n") ? "\n" : ""}.gitnexus/\n`, ".gitignore (.gitnexus/)");
}

// 5. Claude Desktop: su configuración es global, así que solo se imprime el fragmento.
console.log("\nClaude Desktop (pégalo a mano en claude_desktop_config.json → mcpServers):");
console.log(JSON.stringify({ forgex: mcpJsonEntry }, null, 2));
console.log(`\nListo. Comprueba con:  node ${join(here, "verify.mjs")} ${project}`);
