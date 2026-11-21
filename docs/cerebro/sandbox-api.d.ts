/**
 * API que ve la IA dentro de `execute_typescript` (Cerebro Fase 0).
 *
 * Convenciones reales de pctx (ver `crates/pctx_codegen`):
 *  - Cada servidor MCP downstream es un `namespace` en PascalCase (`echo` → `Echo`).
 *  - Cada herramienta es una función `async` en camelCase (`delete_item` → `deleteItem`).
 *  - Un solo parámetro `input` con el esquema de entrada; sin parámetro si no hay campos.
 *  - Los errores de herramienta se lanzan como excepciones; si el mensaje contiene
 *    `CEREBRO_ERROR:{...}`, el servidor lo devuelve como `error` estructurado.
 *
 * Este archivo es un ejemplo con los fixtures `echo` y `destructive`; el real lo genera
 * `get_function_details` a partir del catálogo. Se valida con
 * `tsc --noEmit --strict docs/cerebro/sandbox-api.d.ts`.
 */

declare namespace Echo {
  type AddInput = { a: number; b: number };
  /** Add two numbers. */
  function add(input: AddInput): Promise<any>;

  type EchoInput = { text: string };
  /** Echo the given text back. */
  function echo(input: EchoInput): Promise<any>;
}

declare namespace Destructive {
  type ReadItemInput = { id: string };
  /** Read an item by id. */
  function readItem(input: ReadItemInput): Promise<any>;

  type DeleteItemInput = { id: string };
  /** Delete an item by id. Requiere aprobación humana (NEEDS_APPROVAL). */
  function deleteItem(input: DeleteItemInput): Promise<any>;
}

/**
 * Punto de entrada que define el código de la IA. Lo que devuelve es lo ÚNICO que llega
 * al modelo (recortado a `limits.max_result_bytes`): filtra y agrega aquí.
 */
declare function run(): Promise<unknown>;

/** Código de error estable que devuelve `execute_typescript` en `error.code`. */
type CerebroErrorCode =
  | "TOOL_NOT_FOUND"
  | "INVALID_INPUT"
  | "NEEDS_APPROVAL"
  | "DENIED"
  | "TOOL_ERROR"
  | "TIMEOUT"
  | "LIMIT_EXCEEDED"
  | "SERVER_UNAVAILABLE";

/** Respuesta JSON de `execute_typescript` (texto del contenido MCP). */
interface ExecuteResponse {
  success: boolean;
  output: unknown;
  stdout: string;
  error?: { code: CerebroErrorCode; message: string; approval_token?: string; suggestions?: string[] } | string;
  calls: { tool: string; ms: number; ok: boolean; decision: string; result_bytes: number }[];
  truncated: boolean;
}
