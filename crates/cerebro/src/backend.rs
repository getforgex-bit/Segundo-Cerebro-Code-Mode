//! Cableado del servidor detrás de traits.
//!
//! El servidor solo conoce [`Backend`], que agrupa los traits de `cerebro_types`
//! (store y búsqueda de WS2) y [`HubApi`], un trait local que refleja la API de
//! `cerebro_hub::Hub` (WS3). Al integrar, `HubApi` se implementa para
//! `Arc<cerebro_hub::Hub>` en pocas líneas y los mocks dejan de usarse.

use std::future::Future;
use std::sync::Arc;

use cerebro_types::{AuditLog, CallSummary, Catalog, Detail, ToolHit, ToolSearch};
use pctx_code_mode::model::CallbackConfig;
use pctx_code_mode::registry::PctxRegistry;

/// Resultado de sincronizar el catálogo (espejo de `cerebro_hub::SyncReport`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub servers_ok: Vec<String>,
    pub servers_failed: Vec<(String, String)>,
    pub tools: usize,
}

/// Llamadas acumuladas durante una ejecución (espejo de `cerebro_hub::ExecutionScope`).
pub trait ExecScope: Send + Sync {
    fn calls(&self) -> Vec<CallSummary>;
    /// Bytes de resultados intermedios que se quedaron en el sandbox.
    fn raw_bytes(&self) -> u64;
}

/// Lo que el servidor necesita del hub.
pub trait HubApi: Send + Sync + 'static {
    /// Arranca los servidores downstream y vuelca su `tools/list` en el catálogo.
    fn sync_catalog(&self) -> impl Future<Output = anyhow::Result<SyncSummary>> + Send;
    /// Una config de callback por herramienta catalogada (para `CodeMode::add_callback`).
    fn callback_configs(&self) -> anyhow::Result<Vec<CallbackConfig>>;
    /// Registry nuevo por ejecución, con política, aprobaciones y auditoría por llamada.
    fn build_registry(
        &self,
        execution_id: &str,
        approvals: Vec<String>,
    ) -> anyhow::Result<(PctxRegistry, Box<dyn ExecScope>)>;
    fn shutdown(&self) -> impl Future<Output = ()> + Send;
}

/// Formateador de resultados de búsqueda (`cerebro_search::format_hits` en la integración).
pub type FormatHits = fn(&[ToolHit], Detail) -> String;

/// Dependencias del servidor.
pub struct Backend<H: HubApi> {
    pub catalog: Arc<dyn Catalog>,
    pub search: Arc<dyn ToolSearch>,
    pub audit: Arc<dyn AuditLog>,
    pub format_hits: FormatHits,
    pub hub: Arc<H>,
}

// ───────────────────────── Hub real (WS3) ─────────────────────────

impl ExecScope for cerebro_hub::ExecutionScope {
    fn calls(&self) -> Vec<CallSummary> {
        cerebro_hub::ExecutionScope::calls(self)
    }
    fn raw_bytes(&self) -> u64 {
        cerebro_hub::ExecutionScope::raw_bytes(self)
    }
}

/// Adaptador de `Arc<cerebro_hub::Hub>` al trait local [`HubApi`].
pub struct RealHub(pub Arc<cerebro_hub::Hub>);

impl HubApi for RealHub {
    async fn sync_catalog(&self) -> anyhow::Result<SyncSummary> {
        let r = self.0.sync_catalog().await?;
        for w in &r.warnings {
            tracing::warn!("{w}");
        }
        Ok(SyncSummary {
            servers_ok: r.servers_ok,
            servers_failed: r.servers_failed,
            tools: r.tools,
        })
    }
    fn callback_configs(&self) -> anyhow::Result<Vec<CallbackConfig>> {
        self.0.callback_configs()
    }
    fn build_registry(
        &self,
        execution_id: &str,
        approvals: Vec<String>,
    ) -> anyhow::Result<(PctxRegistry, Box<dyn ExecScope>)> {
        let (reg, scope) = self.0.build_registry(execution_id, approvals)?;
        Ok((reg, Box::new(scope)))
    }
    async fn shutdown(&self) {
        self.0.shutdown().await;
    }
}

/// Backend de producción: `Store` + `SqliteToolSearch` + `Hub` reales.
/// Debe llamarse dentro del runtime tokio principal (el hub guarda su `Handle`).
pub fn real_backend(
    paths: &crate::config::ProjectPaths,
    cfg: &crate::config::CerebroConfig,
) -> anyhow::Result<Backend<RealHub>> {
    let store = cerebro_store::Store::open(&cerebro_store::StorePaths {
        brain_db: paths.brain_db.clone(),
        hub_db: paths.hub_db.clone(),
    })?;
    let store = Arc::new(store);
    let hub = cerebro_hub::Hub::new(
        cerebro_hub::HubConfig {
            servers: cfg.servers.clone(),
            approval_mode: cfg.approvals.mode,
            ..cerebro_hub::HubConfig::default()
        },
        store.clone(),
        store.clone(),
        store.clone(),
        cerebro_policy::Policy::new(cfg.policy.clone()),
    );
    Ok(Backend {
        catalog: store.clone(),
        search: Arc::new(cerebro_search::SqliteToolSearch::new((*store).clone())),
        audit: store,
        format_hits: cerebro_search::format_hits,
        hub: Arc::new(RealHub(hub)),
    })
}
