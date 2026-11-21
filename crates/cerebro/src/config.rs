//! Config de la Fase 0: `.cerebro/config.json` (ver `coordination/CONTRACTS.md` §2).
//!
//! Hasta integrar WS3, `servers` y `policy` se parsean con tipos locales que reflejan
//! la forma de `cerebro_hub::DownstreamServer` y `cerebro_policy::PolicyConfig`.
//! En la integración se sustituyen por esos tipos (mismo JSON).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

/// Raíz de `.cerebro/config.json`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CerebroConfig {
    #[serde(default)]
    pub servers: Vec<ServerEntry>,
    #[serde(default)]
    pub policy: PolicySection,
    #[serde(default)]
    pub limits: Limits,
    /// Modo de aprobación (por defecto `out_of_band`). Se pasa al hub.
    #[serde(default)]
    pub approvals: ApprovalsSection,
}

/// Bloque `"approvals": { "mode": "out_of_band" | "client_confirm" }`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct ApprovalsSection {
    #[serde(default)]
    pub mode: cerebro_types::ApprovalMode,
}

/// Servidor downstream (espejo de `cerebro_hub::DownstreamServer`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ServerEntry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

/// Sección de política (espejo de `cerebro_policy::PolicyConfig`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct PolicySection {
    /// `allow` | `confirm` | `deny`. Por defecto `confirm`.
    pub default_unknown: Option<String>,
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
}

/// Límites de `execute_typescript`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub timeout_ms: u64,
    pub max_result_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout_ms: 10_000,
            max_result_bytes: 20_000,
        }
    }
}

/// Rutas derivadas del directorio del proyecto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPaths {
    pub project: PathBuf,
    pub config: PathBuf,
    pub brain_db: PathBuf,
    pub hub_db: PathBuf,
}

impl ProjectPaths {
    /// `config` por defecto: `<project>/.cerebro/config.json`.
    pub fn new(project: &Path, config: Option<&Path>) -> Self {
        let dot = project.join(".cerebro");
        Self {
            project: project.to_path_buf(),
            config: config.map_or_else(|| dot.join("config.json"), Path::to_path_buf),
            brain_db: dot.join("brain.db"),
            hub_db: dot.join("hub.db"),
        }
    }
}

impl CerebroConfig {
    pub fn from_json(text: &str) -> anyhow::Result<Self> {
        let cfg: Self = serde_json::from_str(text).context("config.json inválido")?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Lee la config. Si el archivo no existe, devuelve la config vacía (sin servidores).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                Self::from_json(&text).with_context(|| format!("leyendo {}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(path = %path.display(), "config no encontrada; se arranca sin servidores");
                Ok(Self::default())
            }
            Err(e) => Err(e).with_context(|| format!("leyendo {}", path.display())),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        for s in &self.servers {
            anyhow::ensure!(!s.name.trim().is_empty(), "servidor sin nombre");
            anyhow::ensure!(
                seen.insert(s.name.as_str()),
                "servidor duplicado: {}",
                s.name
            );
        }
        for rule in self
            .policy
            .default_unknown
            .iter()
            .chain(self.policy.overrides.values())
        {
            anyhow::ensure!(
                matches!(rule.as_str(), "allow" | "confirm" | "deny"),
                "regla de política desconocida: {rule}"
            );
        }
        anyhow::ensure!(
            self.limits.max_result_bytes > 0,
            "max_result_bytes debe ser > 0"
        );
        anyhow::ensure!(self.limits.timeout_ms > 0, "timeout_ms debe ser > 0");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_contract_example() {
        let cfg = CerebroConfig::from_json(
            r#"{
              "servers": [{ "name": "echo", "command": "node", "args": ["packages/fixtures/dist/echo.js"] }],
              "policy": { "default_unknown": "confirm", "overrides": { "echo.*": "allow" } },
              "limits": { "timeout_ms": 10000, "max_result_bytes": 20000 }
            }"#,
        )
        .unwrap();
        assert_eq!(cfg.servers.len(), 1);
        assert_eq!(cfg.servers[0].args, vec!["packages/fixtures/dist/echo.js"]);
        assert_eq!(cfg.policy.overrides["echo.*"], "allow");
        assert_eq!(cfg.limits, Limits::default());
    }

    #[test]
    fn defaults_when_sections_missing() {
        let cfg = CerebroConfig::from_json("{}").unwrap();
        assert!(cfg.servers.is_empty());
        assert_eq!(cfg.limits.max_result_bytes, 20_000);
    }

    #[test]
    fn rejects_duplicates_and_bad_rules() {
        let dup = r#"{"servers":[{"name":"a","command":"x"},{"name":"a","command":"y"}]}"#;
        assert!(CerebroConfig::from_json(dup).is_err());
        let bad = r#"{"policy":{"default_unknown":"maybe"}}"#;
        assert!(CerebroConfig::from_json(bad).is_err());
    }

    #[test]
    fn approval_mode_parsing() {
        use cerebro_types::ApprovalMode;
        let d = CerebroConfig::from_json("{}").unwrap();
        assert_eq!(d.approvals.mode, ApprovalMode::OutOfBand);
        let c = CerebroConfig::from_json(r#"{"approvals":{"mode":"client_confirm"}}"#).unwrap();
        assert_eq!(c.approvals.mode, ApprovalMode::ClientConfirm);
        assert!(CerebroConfig::from_json(r#"{"approvals":{"mode":"nope"}}"#).is_err());
    }

    #[test]
    fn project_paths() {
        let p = ProjectPaths::new(Path::new("/p"), None);
        assert_eq!(p.config, Path::new("/p/.cerebro/config.json"));
        assert_eq!(p.brain_db, Path::new("/p/.cerebro/brain.db"));
        assert_eq!(p.hub_db, Path::new("/p/.cerebro/hub.db"));
    }
}
