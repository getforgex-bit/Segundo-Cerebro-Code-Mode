// Servidor MCP mínimo (JSON-RPC por stdio, sin dependencias) con nombres que colisionan en TS:
// `create_issue` y `createIssue` generan ambos `createIssue`.
import readline from "node:readline";

const tool = (name) => ({
  name,
  description: `tool ${name}`,
  inputSchema: { type: "object", properties: {} },
  annotations: { readOnlyHint: true },
});
const rl = readline.createInterface({ input: process.stdin });
const send = (m) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...m }) + "\n");
rl.on("line", (line) => {
  const msg = JSON.parse(line);
  if (msg.id === undefined) return; // notificaciones
  if (msg.method === "initialize") {
    send({
      id: msg.id,
      result: {
        protocolVersion: msg.params.protocolVersion,
        capabilities: { tools: {} },
        serverInfo: { name: "collide", version: "0.0.0" },
      },
    });
  } else if (msg.method === "tools/list") {
    send({ id: msg.id, result: { tools: [tool("createIssue"), tool("create_issue"), tool("other")] } });
  } else {
    send({ id: msg.id, error: { code: -32601, message: "method not found" } });
  }
});
