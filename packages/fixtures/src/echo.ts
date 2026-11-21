#!/usr/bin/env node
/**
 * Fixture MCP `echo` (CONTRACTS.md §5).
 * - `echo({text})` → texto + structuredContent `{text}`; readOnlyHint.
 * - `add({a,b})` → `{sum}`; readOnlyHint.
 */
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const server = new McpServer(
  { name: "echo", version: "0.0.0" },
  { instructions: "Fixture de pruebas de Cerebro: devuelve lo que recibe." },
);

server.registerTool(
  "echo",
  {
    title: "Echo",
    description: "Echoes the input text. Returns the same text it receives.",
    inputSchema: { text: z.string().describe("Text to echo back") },
    outputSchema: { text: z.string() },
    annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
  },
  async ({ text }) => ({
    content: [{ type: "text", text }],
    structuredContent: { text },
  }),
);

server.registerTool(
  "add",
  {
    title: "Add",
    description: "Adds two numbers and returns their sum.",
    inputSchema: { a: z.number(), b: z.number() },
    outputSchema: { sum: z.number() },
    annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
  },
  async ({ a, b }) => {
    const sum = a + b;
    return {
      content: [{ type: "text", text: String(sum) }],
      structuredContent: { sum },
    };
  },
);

await server.connect(new StdioServerTransport());
