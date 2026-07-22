//! `mulberry32-v1`: pure Rust parity port of `mulberry32` in
//! `src/services/sampleData.ts`.
//!
//! The TypeScript reference operates on 32-bit lanes (`| 0`, `>>>`,
//! `Math.imul`), so every step maps to `u32` wrapping arithmetic and the raw
//! output word is bit-identical. The seed is truncated to 32 bits exactly as
//! `seed >>> 0` does, so `2^32 + n` reduces to `n`.

pub const PRNG_CONTRACT_VERSION: &str = "mulberry32-v1";

pub struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    /// Seed truncated to 32 bits (mirrors the reference `seed >>> 0`).
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Truncate an arbitrary integer seed to 32 bits, matching `>>> 0` on a
    /// JavaScript number (the low 32 bits of the two's-complement value).
    pub fn from_truncated(seed: i64) -> Self {
        Self::new(seed as u32)
    }

    /// The raw 32-bit output word (the value the reference divides by 2^32).
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        let s = self.state;
        let mut t = (s ^ (s >> 15)).wrapping_mul(s | 1);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61)) ^ t;
        t ^ (t >> 14)
    }

    /// A float in [0, 1), exactly `next_u32() / 2^32` as the reference returns.
    pub fn next_f64(&mut self) -> f64 {
        self.next_u32() as f64 / 4_294_967_296.0
    }
}
