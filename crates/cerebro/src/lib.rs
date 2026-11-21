//! Servidor MCP `cerebro` (WS4): 6 meta-herramientas sobre el motor Code Mode de pctx.
pub mod backend;
pub mod config;
#[cfg(any(test, feature = "mock"))]
pub mod mock;
pub mod server;
pub mod tools;
