import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { binExists, callText, fixturesBuilt, mockBinary, startSession, type Session } from "./harness.js";

// Sin binario, o sin fixtures cuando hay hub real, no hay falso verde: se marca como omitido
// con el motivo (CONTRACTS §6).
const runnable = binExists && (mockBinary || fixturesBuilt);
const d = runnable ? describe : describe.skip;
// En CI (CEREBRO_E2E_REQUIRED=1) un E2E que no puede ejecutarse es un fallo, no un "omitido".
if (!runnable && process.env.CEREBRO_E2E_REQUIRED === "1") {
  throw new Error(
    `E2E requerido pero no ejecutable: binario=${binExists} fixtures=${fixturesBuilt} mock=${mockBinary}`,
  );
}
if (!runnable) {
  console.warn(
    `E2E omitido: binario=${binExists} fixtures=${fixturesBuilt} mock=${mockBinary}. ` +
      "Compila con `cargo build -p cerebro` y `pnpm -C packages/fixtures build`.",
  );
}

const DELETE = "async function run(){ return await Destructive.deleteItem({ id: '1' }); }";

d("cerebro E2E (client_confirm)", () => {
  let s: Session;
  beforeAll(async () => {
    s = await startSession({ approvalsMode: "client_confirm" });
  });
  afterAll(async () => {
    await s?.close();
  });

  it("tools/list: 6 herramientas y tamaño registrado ≤ 8000 bytes", async () => {
    const { tools } = await s.client.listTools();
    expect(tools.map((t) => t.name).sort()).toEqual(
      [
        "cerebro_primer",
        "cerebro_recall",
        "cerebro_remember",
        "execute_typescript",
        "get_function_details",
        "search_functions",
      ].sort(),
    );
    const bytes = Buffer.byteLength(JSON.stringify(tools));
    console.log(`TOOLS_LIST_BYTES=${bytes} approx_tokens=${Math.round(bytes / 4)}`);
    expect(bytes).toBeLessThanOrEqual(8000);
  });

  it("search_functions('echo text') devuelve Echo.echo primero", async () => {
    const out = await callText(s, "search_functions", { query: "echo text" });
    expect(out.split("\n")[0]).toContain("Echo.echo");
  });

  it("get_function_details acepta Namespace.fn y server.tool", async () => {
    const a = await callText(s, "get_function_details", { functions: ["Echo.echo"] });
    const b = await callText(s, "get_function_details", { functions: ["echo.echo"] });
    expect(a).toContain("echo");
    expect(b).toBe(a);
  });

  it("execute_typescript: dos llamadas, solo un resumen vuelve", async () => {
    const r = await s.exec(
      `async function run(){
        const a = await Echo.echo({ text: "hola" });
        const b = await Echo.add({ a: 2, b: 3 });
        return { ok: true, n: 2 };
      }`,
    );
    expect(r.success).toBe(true);
    expect(r.calls).toHaveLength(2);
    expect(JSON.stringify(r.output)).toContain("ok");
  });

  it("delete_item: NEEDS_APPROVAL y luego ok con el token (client_confirm)", async () => {
    const r1 = await s.exec(DELETE);
    expect(r1.success).toBe(false);
    const err = r1.error as { code: string; approval_token: string };
    expect(err.code).toBe("NEEDS_APPROVAL");
    expect(err.approval_token).toBeTruthy();
    const r2 = await s.exec(DELETE, [err.approval_token]);
    expect(r2.success).toBe(true);
  });

  it("error de tipos: diagnóstico legible, sin colgarse", async () => {
    const r = await s.exec("async function run(){ const x: number = 'no'; return x; }");
    expect(r.success).toBe(false);
    expect(JSON.stringify(r.error ?? r.output)).toMatch(/number|string|type|TS\d+/i);
  });

  it("stubs de memoria y primer real", async () => {
    expect(await callText(s, "cerebro_recall", { query: "x" })).toContain("phase 2");
    const p = await callText(s, "cerebro_primer", {});
    expect(p).toContain("echo");
  });
});

d("cerebro E2E (out_of_band)", () => {
  let s: Session;
  beforeAll(async () => {
    s = await startSession({ approvalsMode: "out_of_band" });
  });
  afterAll(async () => {
    await s?.close();
  });

  it("reenviar el token SIN aprobación humana sigue dando NEEDS_APPROVAL", async () => {
    const r1 = await s.exec(DELETE);
    const token = (r1.error as { approval_token: string }).approval_token;
    const r2 = await s.exec(DELETE, [token]);
    expect(r2.success).toBe(false);
    expect((r2.error as { code: string }).code).toBe("NEEDS_APPROVAL");
  });

  it("tras la aprobación humana (fila en brain.db) funciona, una sola vez", async () => {
    const r1 = await s.exec(DELETE);
    const token = (r1.error as { approval_token: string }).approval_token;
    s.humanApprove(token);
    const r2 = await s.exec(DELETE, [token]);
    expect(r2.success).toBe(true);
    const r3 = await s.exec(DELETE, [token]);
    expect(r3.success).toBe(false);
  });
});

d("cerebro E2E (timeout)", () => {
  let s: Session;
  beforeAll(async () => {
    s = await startSession({ approvalsMode: "client_confirm", limits: { timeout_ms: 2000 } });
  });
  afterAll(async () => {
    await s?.close();
  });

  it("while(true){} con timeout_ms=2000 devuelve error en < 5 s", async () => {
    const t0 = Date.now();
    const r = await s.exec("async function run(){ while (true) {} }");
    const dt = Date.now() - t0;
    expect(r.success).toBe(false);
    expect((r.error as { code: string }).code).toBe("TIMEOUT");
    expect(dt).toBeLessThan(5000);
    // El servidor sigue vivo tras el timeout.
    const ok = await s.exec("async function run(){ return await Echo.echo({ text: 'vivo' }); }");
    expect(ok.success).toBe(true);
  });
});
