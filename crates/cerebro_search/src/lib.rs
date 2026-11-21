//! Búsqueda de herramientas de Cerebro (WS2) sobre el catálogo de [`cerebro_store::Store`].
//!
//! Dos rankings sobre `hub.tools` se combinan con RRF (k = 60):
//! 1. `tools_fts` (porter): BM25 con pesos (name 10, title 5, description 2, search_text 3).
//! 2. `tools_trgm` (trigram): tolera erratas y subcadenas (`isue` → `issue`).

use std::collections::{BTreeMap, HashMap};

use cerebro_store::{Store, map_tool_row, split_words, tool_columns};
use cerebro_types::{Detail, Result, SearchOpts, ToolHit, ToolRecord, ToolSearch};
use rusqlite::params;

/// Constante de RRF.
const RRF_K: f64 = 60.0;
const DEFAULT_LIMIT: usize = 8;
const MAX_LIMIT: usize = 25;
const MIN_CANDIDATES: usize = 50;

/// Búsqueda FTS5 (BM25 + trigram combinados con RRF) sobre `hub.tools`.
#[derive(Debug, Clone)]
pub struct SqliteToolSearch {
    store: Store,
}

impl SqliteToolSearch {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

/// Expresión MATCH para `tools_fts`: cada término entre comillas, unidos con OR.
fn fts_expr(terms: &[String]) -> String {
    terms
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Expresión MATCH para `tools_trgm`: trigramas de cada término de 3+ caracteres, con OR.
fn trigram_expr(terms: &[String]) -> Option<String> {
    let mut grams: Vec<String> = Vec::new();
    for t in terms {
        let chars: Vec<char> = t.chars().collect();
        for w in chars.windows(3) {
            let g: String = w.iter().collect();
            if !grams.contains(&g) {
                grams.push(g);
            }
        }
    }
    if grams.is_empty() {
        None
    } else {
        Some(
            grams
                .iter()
                .map(|g| format!("\"{g}\""))
                .collect::<Vec<_>>()
                .join(" OR "),
        )
    }
}

impl SqliteToolSearch {
    /// Ids ordenados por relevancia en una FTS (`tools_fts` o `tools_trgm`).
    fn ranked_ids(
        &self,
        table: &str,
        weights: &str,
        expr: &str,
        server: Option<&str>,
        candidates: usize,
    ) -> Result<Vec<i64>> {
        let conn = self.store.conn();
        let sql = format!(
            "SELECT t.id FROM hub.{table} JOIN hub.tools t ON t.id = {table}.rowid \
             WHERE {table} MATCH ?1 AND t.available = 1 AND (?2 IS NULL OR t.server = ?2) \
             ORDER BY bm25({table}, {weights}), t.server, t.name LIMIT ?3"
        );
        let mut st = conn.prepare(&sql)?;
        let ids = st
            .query_map(params![expr, server, candidates as i64], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;
        Ok(ids)
    }

    fn tools_by_ids(&self, ids: &[i64]) -> Result<HashMap<i64, ToolRecord>> {
        let conn = self.store.conn();
        let mut st = conn.prepare(&format!(
            "SELECT {} FROM hub.tools WHERE id = ?1",
            tool_columns()
        ))?;
        let mut out = HashMap::new();
        for &id in ids {
            out.insert(id, st.query_row([id], map_tool_row)?);
        }
        Ok(out)
    }

    fn first_by_name(&self, server: Option<&str>, limit: usize) -> Result<Vec<ToolHit>> {
        let conn = self.store.conn();
        let mut st = conn.prepare(&format!(
            "SELECT {} FROM hub.tools WHERE available = 1 AND (?1 IS NULL OR server = ?1) \
             ORDER BY server, name LIMIT ?2",
            tool_columns()
        ))?;
        let hits = st
            .query_map(params![server, limit as i64], map_tool_row)?
            .map(|r| r.map(|tool| ToolHit { tool, score: 0.0 }))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(hits)
    }
}

impl ToolSearch for SqliteToolSearch {
    fn search(&self, query: &str, opts: &SearchOpts) -> Result<Vec<ToolHit>> {
        let limit = if opts.limit == 0 {
            DEFAULT_LIMIT
        } else {
            opts.limit.min(MAX_LIMIT)
        };
        let server = opts.server.as_deref();
        let mut terms = split_words(query);
        let mut seen = std::collections::HashSet::new();
        terms.retain(|t| seen.insert(t.clone()));
        if terms.is_empty() {
            return self.first_by_name(server, limit);
        }
        let candidates = (limit * 4).max(MIN_CANDIDATES);

        let mut rankings: Vec<Vec<i64>> = vec![self.ranked_ids(
            "tools_fts",
            "10.0, 5.0, 2.0, 3.0",
            &fts_expr(&terms),
            server,
            candidates,
        )?];
        if let Some(expr) = trigram_expr(&terms) {
            rankings.push(self.ranked_ids(
                "tools_trgm",
                "10.0, 5.0, 2.0, 3.0",
                &expr,
                server,
                candidates,
            )?);
        }

        let mut scores: BTreeMap<i64, f64> = BTreeMap::new();
        for ranking in &rankings {
            for (rank, id) in ranking.iter().enumerate() {
                *scores.entry(*id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
            }
        }
        let ids: Vec<i64> = scores.keys().copied().collect();
        let mut tools = self.tools_by_ids(&ids)?;
        let mut hits: Vec<ToolHit> = scores
            .into_iter()
            .filter_map(|(id, score)| tools.remove(&id).map(|tool| ToolHit { tool, score }))
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.tool.qualified_name().cmp(&b.tool.qualified_name()))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

/// Primera frase de `desc` o, si es más larga, sus primeros 100 caracteres con «…».
fn short_description(desc: &str) -> String {
    let flat = desc.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut end = flat.len();
    let bytes = flat.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if matches!(b, b'.' | b'!' | b'?') && bytes.get(i + 1).is_none_or(|n| *n == b' ') {
            end = i;
            break;
        }
    }
    let sentence = flat[..end].trim_end();
    if sentence.chars().count() > 100 {
        let cut: String = sentence.chars().take(100).collect();
        format!("{}…", cut.trim_end())
    } else {
        sentence.to_string()
    }
}

fn mark(tool: &ToolRecord) -> &'static str {
    if tool.read_only_hint() == Some(true) {
        "[read]"
    } else if tool.destructive_hint() == Some(true) {
        "[destructive]"
    } else {
        "[write]"
    }
}

/// Texto compacto para `search_functions`: una línea por resultado (ver `CONTRACTS.md` §4).
///
/// El identificador mostrado es `namespace.ts_name`. Mientras `ToolHit` no lleve
/// `ts_namespace`, se usa el nombre del servidor como namespace.
pub fn format_hits(hits: &[ToolHit], detail: Detail) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(hits.len());
    for hit in hits {
        let t = &hit.tool;
        let id = format!("{}.{}", t.server, t.ts_name);
        match detail {
            Detail::Names => lines.push(id),
            Detail::Summary | Detail::Full => {
                let desc = short_description(&t.description);
                let head = if desc.is_empty() {
                    format!("{id} {}", mark(t))
                } else {
                    format!("{id} — {desc} {}", mark(t))
                };
                lines.push(head);
                if detail == Detail::Full
                    && let Some(sig) = t.ts_signature.as_deref().filter(|s| !s.is_empty())
                {
                    lines.push(format!("  {sig}"));
                }
            }
        }
    }
    lines.join("\n")
}
