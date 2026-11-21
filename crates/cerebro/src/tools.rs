//! Definiciones de las 6 meta-herramientas (`coordination/CONTRACTS.md` §3).
//!
//! Los esquemas se escriben a mano (no con `schemars`) para controlar el tamaño de
//! `tools/list`: el objetivo es ≤ 2.000 tokens (≈ bytes / 4). Las descripciones van
//! en inglés y son deliberadamente breves.

use std::sync::Arc;

use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use serde::Deserialize;
use serde_json::{Value, json};

pub const SEARCH_FUNCTIONS: &str = "search_functions";
pub const GET_FUNCTION_DETAILS: &str = "get_function_details";
pub const EXECUTE_TYPESCRIPT: &str = "execute_typescript";
pub const CEREBRO_RECALL: &str = "cerebro_recall";
pub const CEREBRO_REMEMBER: &str = "cerebro_remember";
pub const CEREBRO_PRIMER: &str = "cerebro_primer";

/// `instructions` MCP que el servidor envía en `initialize`.
pub const INSTRUCTIONS: &str = "Use search_functions to find tools, get_function_details for \
signatures, then compose calls in execute_typescript.";

/// Texto de los stubs de memoria (Fase 2).
pub const BRAIN_STUB: &str = "brain not implemented yet (phase 2)";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchArgs {
    pub query: String,
    #[serde(default)]
    pub detail: Option<cerebro_types::Detail>,
    #[serde(default)]
    pub server: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailsArgs {
    pub functions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecuteArgs {
    pub code: String,
    #[serde(default)]
    pub approvals: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecallArgs {
    pub query: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RememberArgs {
    pub kind: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrimerArgs {
    #[serde(default)]
    pub focus: Option<String>,
}

fn obj(v: Value) -> Arc<JsonObject> {
    match v {
        Value::Object(m) => Arc::new(m),
        _ => unreachable!("los esquemas son objetos literales"),
    }
}

fn tool(name: &'static str, description: &'static str, schema: Value, read_only: bool) -> Tool {
    let mut t = Tool::new(name, description, obj(schema));
    if read_only {
        t.annotations = Some(ToolAnnotations::new().read_only(true));
    }
    t
}

/// Las 6 herramientas, en el orden en que se anuncian.
pub fn all() -> Vec<Tool> {
    vec![
        tool(
            SEARCH_FUNCTIONS,
            "Search available tool functions. One line per match: \
             `namespace.fn — summary [read|write|destructive]`.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "detail": { "enum": ["names", "summary", "full"] },
                    "server": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 25 }
                },
                "required": ["query"]
            }),
            true,
        ),
        tool(
            GET_FUNCTION_DETAILS,
            "Get TypeScript signatures for functions (e.g. [\"echo.echo\"]). \
             Call before using them in execute_typescript.",
            json!({
                "type": "object",
                "properties": {
                    "functions": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["functions"]
            }),
            true,
        ),
        tool(
            EXECUTE_TYPESCRIPT,
            "Run TypeScript in a sandbox. Define `async function run() { ... return x; }`; \
             call tools as `await Namespace.fn(args)`. Only the return value reaches you, \
             so filter and aggregate in code. Returns JSON \
             {success, output, stdout, error?, calls, truncated}. If error.code is \
             NEEDS_APPROVAL, ask the user, then rerun with approvals: [error.approval_token].",
            json!({
                "type": "object",
                "properties": {
                    "code": { "type": "string" },
                    "approvals": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["code"]
            }),
            false,
        ),
        tool(
            CEREBRO_RECALL,
            "Search project memory: decisions, conventions, docs and code.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "scope": { "type": "string" },
                    "limit": { "type": "integer" }
                },
                "required": ["query"]
            }),
            true,
        ),
        tool(
            CEREBRO_REMEMBER,
            "Save a project memory (decision, convention, fact, task).",
            json!({
                "type": "object",
                "properties": {
                    "kind": { "type": "string" },
                    "title": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["kind", "title", "body"]
            }),
            false,
        ),
        tool(
            CEREBRO_PRIMER,
            "Project overview: connected tool servers, their status and tool counts. \
             Call at session start.",
            json!({
                "type": "object",
                "properties": { "focus": { "type": "string" } }
            }),
            true,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_unique_tools() {
        let tools = all();
        assert_eq!(tools.len(), 6);
        let mut names: Vec<_> = tools.iter().map(|t| t.name.to_string()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 6);
    }
}
