// Smoke test de FORGEX Stack. Uso: node verify.mjs <ruta-proyecto>
// Comprueba: pctx (Code Mode) por stdio, memoria vía execute_typescript y tamaño de tools/list (pctx vs GitNexus directo).
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";

const project = resolve(process.argv[2] ?? process.cwd());
const pctxConfig = join(project, "pctx.json");
const memoryFile = join(project, ".forgex", "memory.jsonl");
const tokens = (obj) => Math.round(JSON.stringify(obj).length / 4);
let failed = false;
const ok = (msg) => console.log(`  ✔ ${msg}`);
const bad = (msg) => { failed = true; console.log(`  ✘ ${msg}`); };

async function connect(command, args, cwd) {
  const client = new Client({ name: "forgex-verify", version: "1.0.0" });
  await client.connect(new StdioClientTransport({ command, args, cwd, stderr: "ignore" }));
  return client;
}
const text = (res) => (res.content ?? []).map((c) => c.text ?? "").join("\n");

console.log(`FORGEX Stack · verificación de ${project}\n`);

// 1. pctx (Code Mode)
console.log("1) pctx (Code Mode)");
if (!existsSync(pctxConfig)) bad(`falta ${pctxConfig} (ejecuta setup primero)`);
else {
  const pctx = await connect("pctx", ["--config", pctxConfig, "code-mode", "start", "--stdio", "--no-banner"], project);
  const { tools } = await pctx.listTools();
  const names = tools.map((t) => t.name);
  ok(`herramientas: ${names.join(", ")} (~${tokens(tools)} tokens fijos)`);
  const listTool = names.find((n) => /^list_/.test(n));
  const execTool = names.find((n) => /execute_typescript/.test(n));
  if (!execTool) bad("pctx no expone execute_typescript");
  else {
    const listing = listTool ? text(await pctx.callTool({ name: listTool, arguments: {} })) : "";
    ok(`${listTool} bajo demanda: ~${Math.round(listing.length / 4)} tokens (evítalo: usa la chuleta de AGENTS.forgex.md)`);
    const ns = /\bMemory\b/.test(listing) ? "Memory" : "memory";
    const withGn = /\bGitnexus\b/.test(listing);
    // Una sola ejecución que compone memoria + grafo de código: solo vuelve el resumen.
    const code = `async function run() {
  await ${ns}.createEntities({ entities: [{ name: "forgex-verify", entityType: "test", observations: ["verificado ${new Date().toISOString()}"] }] });
  const found = await ${ns}.searchNodes({ query: "forgex-verify" });
  ${withGn ? 'const flows = await Gitnexus.query({ search_query: "delete item", limit: 3 });' : "const flows = null;"}
  return { memoria: JSON.stringify(found).includes("forgex-verify"), gitnexus: flows === null ? "no configurado" : JSON.stringify(flows).length > 2 };
}`;
    const res = await pctx.callTool({ name: execTool, arguments: { code } });
    const out = text(res);
    if (res.isError || !/"memoria":\s*true/.test(out)) bad(`execute_typescript falló:\n${out.slice(0, 800)}`);
    else {
      ok(`execute_typescript → memoria creada y leída (namespace ${ns})`);
      if (withGn) /"gitnexus":\s*true/.test(out) ? ok("execute_typescript → Gitnexus.query respondió a través de pctx") : bad(`Gitnexus.query sin resultado:\n${out.slice(0, 600)}`);
    }
  }
  await pctx.close();
  existsSync(memoryFile) ? ok(`memoria persistida en ${memoryFile}`) : bad(`no existe ${memoryFile}`);
}

// 2. GitNexus directo (para decidir si va detrás de pctx)
console.log("\n2) GitNexus directo (solo medición; FORGEX Stack lo usa detrás de pctx)");
if (!existsSync(join(project, ".gitnexus"))) bad("falta .gitnexus/ (ejecuta `gitnexus analyze` en el proyecto)");
else {
  const gn = await connect("gitnexus", ["mcp"], project);
  const { tools } = await gn.listTools();
  const t = tokens(tools);
  ok(`${tools.length} herramientas (~${t} tokens si van directas)`);
  console.log(`  → detrás de pctx cuesta ~0 tokens fijos extra; conectado directo costaría ~${t} por sesión`);
  await gn.close();
}

console.log(failed ? "\nRESULTADO: FALLÓ" : "\nRESULTADO: OK");
process.exit(failed ? 1 : 0);
