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
