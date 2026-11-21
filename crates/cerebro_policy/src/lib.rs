//! Política de Cerebro: decide si una llamada a una herramienta downstream se permite,
//! requiere confirmación humana o se deniega (`CONTRACTS.md` §2).
//!
//! Precedencia (de mayor a menor):
//! 1. **Override** de la config. Gana el patrón más específico:
//!    `"server.tool"` > `"server.*"` > `"*"`.
//! 2. `readOnlyHint == true` **y** `destructiveHint == true` (conflicto) → [`Decision::Confirm`]:
//!    ante anotaciones incoherentes gana lo más restrictivo.
//! 3. `readOnlyHint == true` → [`Decision::Allow`].
//! 4. `destructiveHint == true` → [`Decision::Confirm`].
//! 5. `default_unknown` (por defecto [`Rule::Confirm`]).
//!
//! La regla de taint (M5) llegará en la Fase 2 y usará [`CallCtx::taint_seen`]; hoy el
//! contexto no influye en la decisión.
//!
//! Nota de seguridad: las anotaciones MCP las declara el propio servidor downstream y no son
//! una garantía. Un override `deny`/`confirm` siempre puede endurecerlas.

use std::collections::BTreeMap;

use cerebro_types::{CallCtx, Decision, ToolRecord};
use serde::Deserialize;

/// Regla configurable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    Allow,
    Confirm,
    Deny,
}

/// Configuración de la política (sección `policy` de `.cerebro/config.json`).
#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyConfig {
    /// Regla para herramientas sin override ni anotación concluyente. Por defecto `Confirm`.
    #[serde(default)]
    pub default_unknown: Option<Rule>,
    /// Claves `"server.tool"`, `"server.*"` o `"*"`; gana la más específica.
    #[serde(default)]
    pub overrides: BTreeMap<String, Rule>,
}

/// Política compilada a partir de [`PolicyConfig`].
#[derive(Debug, Clone)]
pub struct Policy {
    default_unknown: Rule,
    overrides: BTreeMap<String, Rule>,
}

impl Policy {
    pub fn new(cfg: PolicyConfig) -> Self {
        Self {
            default_unknown: cfg.default_unknown.unwrap_or(Rule::Confirm),
            overrides: cfg.overrides,
        }
    }

    /// Decide qué hacer con una llamada a `tool`.
    pub fn decide(&self, tool: &ToolRecord, _ctx: &CallCtx) -> Decision {
        let qualified = tool.qualified_name();
        if let Some((pattern, rule)) = self.find_override(&tool.server, &qualified) {
            return to_decision(rule, &format!("override `{pattern}` for {qualified}"));
        }
        if tool.read_only_hint() == Some(true) && tool.destructive_hint() == Some(true) {
            return Decision::Confirm {
                reason: format!(
                    "{qualified} has conflicting annotations (readOnlyHint and destructiveHint)"
                ),
            };
        }
        if tool.read_only_hint() == Some(true) {
            return Decision::Allow;
        }
        if tool.destructive_hint() == Some(true) {
            return Decision::Confirm {
                reason: format!("{qualified} is marked destructive (destructiveHint)"),
            };
        }
        to_decision(
            self.default_unknown,
            &format!("{qualified} is not marked read-only (default_unknown)"),
        )
    }

    fn find_override(&self, server: &str, qualified: &str) -> Option<(String, Rule)> {
        [
            qualified.to_string(),
            format!("{server}.*"),
            "*".to_string(),
        ]
        .into_iter()
        .find_map(|k| self.overrides.get(&k).map(|r| (k, *r)))
    }
}

fn to_decision(rule: Rule, reason: &str) -> Decision {
    match rule {
        Rule::Allow => Decision::Allow,
        Rule::Confirm => Decision::Confirm {
            reason: reason.to_string(),
        },
        Rule::Deny => Decision::Deny {
            reason: reason.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn tool(server: &str, name: &str, annotations: Value) -> ToolRecord {
        ToolRecord {
            server: server.into(),
            name: name.into(),
            ts_name: name.into(),
            title: None,
            description: String::new(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            annotations,
            ts_signature: None,
            approx_tokens: 0,
            schema_hash: String::new(),
            taint_labels: vec![],
        }
    }

    fn policy(v: Value) -> Policy {
        Policy::new(serde_json::from_value(v).unwrap())
    }

    fn is_confirm(d: &Decision) -> bool {
        matches!(d, Decision::Confirm { .. })
    }

    fn is_deny(d: &Decision) -> bool {
        matches!(d, Decision::Deny { .. })
    }

    #[test]
    fn read_only_allows() {
        let p = Policy::new(PolicyConfig::default());
        let t = tool("echo", "echo", json!({"readOnlyHint": true}));
        assert_eq!(p.decide(&t, &CallCtx::default()), Decision::Allow);
    }

    #[test]
    fn destructive_confirms() {
        let p = policy(json!({"default_unknown": "allow"}));
        let t = tool("d", "delete_item", json!({"destructiveHint": true}));
        assert!(is_confirm(&p.decide(&t, &CallCtx::default())));
    }

    #[test]
    fn conflicting_hints_confirm() {
        // Combinación incoherente: gana lo más restrictivo (CONTRACTS §2).
        let p = policy(json!({"default_unknown": "allow"}));
        let t = tool(
            "d",
            "x",
            json!({"readOnlyHint": true, "destructiveHint": true}),
        );
        assert!(is_confirm(&p.decide(&t, &CallCtx::default())));
        // Un override explícito sigue mandando sobre el conflicto.
        let p = policy(json!({"overrides": {"d.x": "allow"}}));
        assert_eq!(p.decide(&t, &CallCtx::default()), Decision::Allow);
    }

    #[test]
    fn unknown_defaults_to_confirm() {
        let p = Policy::new(PolicyConfig::default());
        let t = tool("s", "write", json!({}));
        assert!(is_confirm(&p.decide(&t, &CallCtx::default())));
        let t = tool("s", "write", json!({"readOnlyHint": false}));
        assert!(is_confirm(&p.decide(&t, &CallCtx::default())));
    }

    #[test]
    fn default_unknown_is_configurable() {
        let t = tool("s", "write", json!({}));
        let ctx = CallCtx::default();
        assert_eq!(
            policy(json!({"default_unknown": "allow"})).decide(&t, &ctx),
            Decision::Allow
        );
        assert!(is_deny(
            &policy(json!({"default_unknown": "deny"})).decide(&t, &ctx)
        ));
    }

    #[test]
    fn override_beats_annotations() {
        let p = policy(json!({"overrides": {"echo.echo": "deny", "d.*": "allow"}}));
        let ctx = CallCtx::default();
        assert!(is_deny(&p.decide(
            &tool("echo", "echo", json!({"readOnlyHint": true})),
            &ctx
        )));
        assert_eq!(
            p.decide(
                &tool("d", "delete_item", json!({"destructiveHint": true})),
                &ctx
            ),
            Decision::Allow
        );
    }

    #[test]
    fn most_specific_override_wins() {
        let p = policy(json!({"overrides": {
            "*": "deny",
            "d.*": "confirm",
            "d.read_item": "allow"
        }}));
        let ctx = CallCtx::default();
        assert_eq!(
            p.decide(&tool("d", "read_item", json!({})), &ctx),
            Decision::Allow
        );
        assert!(is_confirm(&p.decide(&tool("d", "other", json!({})), &ctx)));
        assert!(is_deny(&p.decide(
            &tool("echo", "echo", json!({"readOnlyHint": true})),
            &ctx
        )));
    }

    #[test]
    fn server_wildcard_does_not_match_prefix_servers() {
        // `d.*` no debe aplicarse al servidor `dx`.
        let p = policy(json!({"overrides": {"d.*": "deny"}}));
        let t = tool("dx", "read", json!({"readOnlyHint": true}));
        assert_eq!(p.decide(&t, &CallCtx::default()), Decision::Allow);
    }

    #[test]
    fn config_parses_contract_example_and_rejects_unknown() {
        let cfg: PolicyConfig = serde_json::from_value(
            json!({ "default_unknown": "confirm", "overrides": { "echo.*": "allow" } }),
        )
        .unwrap();
        assert_eq!(cfg.default_unknown, Some(Rule::Confirm));
        assert_eq!(cfg.overrides.get("echo.*"), Some(&Rule::Allow));
        assert!(serde_json::from_value::<PolicyConfig>(json!({"overides": {}})).is_err());
        assert!(
            serde_json::from_value::<PolicyConfig>(json!({"default_unknown": "maybe"})).is_err()
        );
        assert_eq!(
            serde_json::from_value::<PolicyConfig>(json!({})).unwrap(),
            PolicyConfig::default()
        );
    }
}
