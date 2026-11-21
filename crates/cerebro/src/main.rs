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
    Ok(cerebro::mock::store_backend(store))
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
