import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { DatabaseSync } from "node:sqlite";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const repoRoot = resolve(here, "../../..");
export const cerebroBin = process.env.CEREBRO_BIN ?? join(repoRoot, "target/debug/cerebro");
export const fixturesDist = join(repoRoot, "packages/fixtures/dist");

export const binExists = existsSync(cerebroBin);
/** Con el hub real, los fixtures hacen falta; con el binario `--features mock` no. */
export const mockBinary = process.env.CEREBRO_MOCK === "1";
export const fixturesBuilt =
  existsSync(join(fixturesDist, "echo.js")) && existsSync(join(fixturesDist, "destructive.js"));

export interface Session {
  client: Client;
  project: string;
  close(): Promise<void>;
  /** Llama a `execute_typescript` y devuelve el JSON de la respuesta. */
  exec(code: string, approvals?: string[]): Promise<ExecResult>;
  /** Simula al humano: aprueba el token directamente en brain.db. */
  humanApprove(token: string): void;
}

export interface ExecResult {
  success: boolean;
  output: unknown;
  stdout: string;
  error?: { code?: string; message?: string; approval_token?: string } | string;
  calls: { tool: string; ms: number; ok: boolean }[];
  truncated: boolean;
}

export async function startSession(opts: {
  approvalsMode?: "out_of_band" | "client_confirm";
  policy?: Record<string, unknown>;
}): Promise<Session> {
  const project = mkdtempSync(join(tmpdir(), "cerebro-e2e-"));
  mkdirSync(join(project, ".cerebro"));
  writeFileSync(
    join(project, ".cerebro/config.json"),
    JSON.stringify({
      servers: [
        { name: "echo", command: "node", args: [join(fixturesDist, "echo.js")] },
        { name: "destructive", command: "node", args: [join(fixturesDist, "destructive.js")] },
      ],
      policy: opts.policy ?? { default_unknown: "confirm", overrides: { "echo.*": "allow" } },
      limits: { timeout_ms: 10000, max_result_bytes: 20000 },
      approvals: { mode: opts.approvalsMode ?? "out_of_band" },
    }),
  );
  const transport = new StdioClientTransport({
    command: cerebroBin,
    args: ["serve", "--stdio", "--project", project],
    stderr: "pipe",
  });
  const client = new Client({ name: "cerebro-e2e", version: "0.0.0" });
  await client.connect(transport);

  const text = async (name: string, args: Record<string, unknown>) => {
    const r = await client.callTool({ name, arguments: args });
    return (r.content as { type: string; text: string }[])[0].text;
  };
  return {
    client,
    project,
    async close() {
      await client.close();
      rmSync(project, { recursive: true, force: true });
    },
    async exec(code, approvals) {
      return JSON.parse(await text("execute_typescript", { code, ...(approvals ? { approvals } : {}) }));
    },
    humanApprove(token) {
      const db = new DatabaseSync(join(project, ".cerebro/brain.db"));
      try {
        const r = db.prepare("UPDATE approvals SET status='approved' WHERE token=?").run(token);
        if (Number(r.changes) !== 1) throw new Error(`token no encontrado: ${token}`);
      } finally {
        db.close();
      }
    },
  };
}

export async function callText(s: Session, name: string, args: Record<string, unknown>): Promise<string> {
  const r = await s.client.callTool({ name, arguments: args });
  return (r.content as { type: string; text: string }[])[0].text;
}
