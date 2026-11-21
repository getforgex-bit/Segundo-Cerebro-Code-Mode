//! CLI de `cerebro`: `serve --stdio` y `sync`. Logs a stderr (stdout es el canal MCP).

use std::path::PathBuf;

use anyhow::Context;
use cerebro::backend::HubApi as _;
use cerebro::config::{CerebroConfig, ProjectPaths};
use cerebro::server::CerebroServer;
use clap::{Parser, Subcommand};
use rmcp::ServiceExt;

#[derive(Parser)]
#[command(
    name = "cerebro",
    version,
    about = "Servidor MCP Code Mode (6 meta-herramientas)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Arranca el servidor MCP.
    Serve {
        /// Transporte stdio (único soportado en la Fase 0).
        #[arg(long)]
        stdio: bool,
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Lista las aprobaciones pendientes.
    Approvals {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Aprueba (como humano, en una terminal interactiva) una aprobación pendiente.
    Approve {
        token: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Sincroniza el catálogo e imprime el resumen.
    Sync {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

/// Construye backend + límites. Con la feature `mock` usa los mocks en memoria; sin ella
/// requiere el hub real (WS3), aún no integrado.
#[cfg(feature = "mock")]
fn backend(
    paths: &ProjectPaths,
    _cfg: &CerebroConfig,
) -> anyhow::Result<cerebro::backend::Backend<cerebro::mock::MockHub>> {
    tracing::warn!("hub MOCK con Store y búsqueda reales (solo desarrollo)");
    let store = cerebro_store::Store::open(&cerebro_store::StorePaths {
        brain_db: paths.brain_db.clone(),
        hub_db: paths.hub_db.clone(),
    })?;
    Ok(cerebro::mock::store_backend(store, _cfg.approvals.mode))
}

#[cfg(not(feature = "mock"))]
fn backend(
    _paths: &ProjectPaths,
    _cfg: &CerebroConfig,
) -> anyhow::Result<cerebro::backend::Backend<NoHub>> {
    anyhow::bail!(
        "cerebro_hub/cerebro_store aún no integrados en este binario; compila con --features mock"
    )
}

#[cfg(not(feature = "mock"))]
enum NoHub {}
#[cfg(not(feature = "mock"))]
impl cerebro::backend::HubApi for NoHub {
    async fn sync_catalog(&self) -> anyhow::Result<cerebro::backend::SyncSummary> {
        match *self {}
    }
    fn callback_configs(&self) -> anyhow::Result<Vec<pctx_code_mode::model::CallbackConfig>> {
        match *self {}
    }
    fn build_registry(
        &self,
        _: &str,
        _: Vec<String>,
    ) -> anyhow::Result<(
        pctx_code_mode::registry::PctxRegistry,
        Box<dyn cerebro::backend::ExecScope>,
    )> {
        match *self {}
    }
    async fn shutdown(&self) {}
}

fn load(
    project: PathBuf,
    config: Option<PathBuf>,
) -> anyhow::Result<(ProjectPaths, CerebroConfig)> {
    let paths = ProjectPaths::new(&project, config.as_deref());
    let cfg = CerebroConfig::load(&paths.config)
        .with_context(|| format!("config {}", paths.config.display()))?;
    Ok((paths, cfg))
}

fn open_store(project: PathBuf) -> anyhow::Result<cerebro_store::Store> {
    let paths = ProjectPaths::new(&project, None);
    cerebro_store::Store::open(&cerebro_store::StorePaths {
        brain_db: paths.brain_db,
        hub_db: paths.hub_db,
    })
}

/// La aprobación debe hacerla un humano: se exige TTY en stdin y stdout, sin escapes.
fn ensure_interactive(stdin_tty: bool, stdout_tty: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        stdin_tty && stdout_tty,
        "la aprobación tiene que hacerla un humano en una terminal interactiva (stdin y stdout \
         deben ser una TTY); no se puede aprobar desde un agente, pipe o script"
    );
    Ok(())
}

fn approve_cmd(token: &str, project: PathBuf) -> anyhow::Result<()> {
    use std::io::{BufRead, IsTerminal, Write};
    ensure_interactive(
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
    )?;
    let store = open_store(project)?;
    let info = cerebro_types::ApprovalStore::list_pending(&store)?
        .into_iter()
        .find(|a| a.token == token)
        .context("no hay una aprobación pendiente con ese token (¿expiró o ya se usó?)")?;
    println!("Herramienta: {}", info.tool);
    println!("Motivo:      {}", info.reason);
    println!("Ejecución:   {}", info.execution_id);
    println!("Expira:      {}", info.expires_at);
    print!("¿Aprobar esta llamada? [y/N] ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    if matches!(line.trim(), "y" | "Y") {
        anyhow::ensure!(
            cerebro_types::ApprovalStore::approve(&store, token)?,
            "no se pudo aprobar (expiró o ya estaba aprobada)"
        );
        println!("Aprobada. Pide a la IA que reintente con approvals: [\"{token}\"].");
    } else {
        println!("No aprobada.");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cerebro=info".into()),
        )
        .init();

    match Cli::parse().cmd {
        Cmd::Serve {
            stdio,
            project,
            config,
        } => {
            anyhow::ensure!(stdio, "solo está soportado --stdio en la Fase 0");
            let (paths, cfg) = load(project, config)?;
            let server = CerebroServer::start(backend(&paths, &cfg)?, cfg.limits).await?;
            let running = server.clone().serve(rmcp::transport::stdio()).await?;
            running.waiting().await?;
            server.shutdown().await;
        }
        Cmd::Approvals { project } => {
            let store = open_store(project)?;
            let list = cerebro_types::ApprovalStore::list_pending(&store)?;
            if list.is_empty() {
                println!("(sin aprobaciones pendientes)");
            }
            for a in list {
                println!(
                    "{}  {}  expira {}  — {}",
                    a.token, a.tool, a.expires_at, a.reason
                );
            }
        }
        Cmd::Approve { token, project } => approve_cmd(&token, project)?,
        Cmd::Sync { project, config } => {
            let (paths, cfg) = load(project, config)?;
            let b = backend(&paths, &cfg)?;
            let report = b.hub.sync_catalog().await?;
            println!(
                "servers_ok={:?} servers_failed={:?} tools={}",
                report.servers_ok, report.servers_failed, report.tools
            );
            b.hub.shutdown().await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ensure_interactive;

    #[test]
    fn approve_refuses_without_tty() {
        assert!(ensure_interactive(false, true).is_err());
        assert!(ensure_interactive(true, false).is_err());
        assert!(ensure_interactive(true, true).is_ok());
    }
}
