#!/usr/bin/env node
/**
 * Fixture MCP `destructive` (CONTRACTS.md §5).
 * - `read_item({id})` → `{id, name, exists}`; readOnlyHint.
 * - `delete_item({id})` → `{id, deleted}`; destructiveHint.
 *
 * Estado en memoria por proceso: los ids `1`, `2` y `3` existen al arrancar.
 * Borrar un id inexistente devuelve `isError: true` (útil para probar TOOL_ERROR).
 */
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const items = new Map<string, string>([
  ["1", "alpha"],
  ["2", "beta"],
  ["3", "gamma"],
]);

const server = new McpServer({ name: "destructive", version: "0.0.0" });

server.registerTool(
  "read_item",
  {
    description: "Reads an item by id. Does not modify anything.",
    inputSchema: { id: z.string() },
    outputSchema: { id: z.string(), name: z.string().nullable(), exists: z.boolean() },
    annotations: { readOnlyHint: true, openWorldHint: false },
  },
  async ({ id }) => {
    const name = items.get(id) ?? null;
    const out = { id, name, exists: name !== null };
    return { content: [{ type: "text", text: JSON.stringify(out) }], structuredContent: out };
  },
);

server.registerTool(
  "delete_item",
  {
    description: "Permanently deletes an item by id. This cannot be undone.",
    inputSchema: { id: z.string() },
    annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: false },
  },
  async ({ id }) => {
    if (!items.delete(id)) {
      return { isError: true, content: [{ type: "text", text: `item ${id} not found` }] };
    }
    return { content: [{ type: "text", text: JSON.stringify({ id, deleted: true }) }] };
  },
);

await server.connect(new StdioServerTransport());
