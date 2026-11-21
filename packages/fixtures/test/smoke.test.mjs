// Smoke test: lanza cada fixture compilado con `node dist/<nombre>.js` y habla MCP por stdio.
import { test } from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const dist = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "dist");

async function connect(name) {
  const client = new Client({ name: "fixtures-smoke", version: "0.0.0" });
  await client.connect(
    new StdioClientTransport({ command: process.execPath, args: [path.join(dist, `${name}.js`)] }),
  );
  return client;
}

test("echo: tools/list y llamadas", async () => {
  const c = await connect("echo");
  try {
    const { tools } = await c.listTools();
    assert.deepEqual(tools.map((t) => t.name).sort(), ["add", "echo"]);
    for (const t of tools) assert.equal(t.annotations?.readOnlyHint, true);
    const r = await c.callTool({ name: "echo", arguments: { text: "hola" } });
    assert.deepEqual(r.structuredContent, { text: "hola" });
    assert.equal(r.content[0].text, "hola");
    const s = await c.callTool({ name: "add", arguments: { a: 2, b: 3 } });
    assert.deepEqual(s.structuredContent, { sum: 5 });
  } finally {
    await c.close();
  }
});

test("destructive: anotaciones, lectura y borrado", async () => {
  const c = await connect("destructive");
  try {
    const { tools } = await c.listTools();
    const byName = Object.fromEntries(tools.map((t) => [t.name, t]));
    assert.equal(byName.read_item.annotations?.readOnlyHint, true);
    assert.equal(byName.delete_item.annotations?.destructiveHint, true);
    const r = await c.callTool({ name: "read_item", arguments: { id: "1" } });
    assert.deepEqual(r.structuredContent, { id: "1", name: "alpha", exists: true });
    const d = await c.callTool({ name: "delete_item", arguments: { id: "1" } });
    assert.ok(!d.isError);
    const again = await c.callTool({ name: "delete_item", arguments: { id: "1" } });
    assert.equal(again.isError, true);
  } finally {
    await c.close();
  }
});
