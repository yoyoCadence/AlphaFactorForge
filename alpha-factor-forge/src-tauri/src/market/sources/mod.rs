//! P07 — market data sources. One module per exchange or vendor, each of
//! them pure: bytes in, rows or a stable rejection code out. The network
//! lives in `market::http`, the recording in `market::provenance`, and the
//! orchestration in `market::ingest`, so a source module can be read (and
//! tested) as a description of what that publisher actually sends.

pub mod binance;
