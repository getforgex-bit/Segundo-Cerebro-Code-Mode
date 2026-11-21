//! Búsqueda de herramientas de Cerebro (WS2) sobre el catálogo de [`cerebro_store::Store`].

use anyhow::bail;
use cerebro_store::Store;
use cerebro_types::{Detail, Result, SearchOpts, ToolHit, ToolSearch};

/// Búsqueda FTS5 (BM25 + trigram combinados con RRF) sobre `hub.tools`.
#[derive(Debug, Clone)]
pub struct SqliteToolSearch {
    #[allow(dead_code)]
    store: Store,
}

impl SqliteToolSearch {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

impl ToolSearch for SqliteToolSearch {
    fn search(&self, _query: &str, _opts: &SearchOpts) -> Result<Vec<ToolHit>> {
        bail!("cerebro_search: search aún no implementado (H4)")
    }
}

/// Texto compacto para `search_functions`: una línea por resultado (ver `CONTRACTS.md` §4).
pub fn format_hits(hits: &[ToolHit], detail: Detail) -> String {
    let _ = (hits, detail);
    String::new()
}
