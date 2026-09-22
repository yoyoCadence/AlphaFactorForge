// STUB — Phase C (Minimal AI Strategy Lab). NOT active in Phase A.
// CRITICAL invariants (enforced when implemented locally):
//   - The frontend NEVER calls Claude/OpenAI/Anthropic directly.
//   - The API key is read from the OS keychain HERE, never sent to frontend.
//   - AI returns ONLY a JSON Strategy DSL; it is validated by the whitelist
//     compiler before it can become a strategy_def. No code execution, ever.

use crate::error::{AppError, AppResult};

/// Phase C: build the prompt (Train-only context), read key from keychain,
/// call the provider, return RAW response for the frontend to preview.
/// Validation + approve happen as separate explicit steps (no auto-queue).
#[tauri::command]
pub fn generate_strategy_dsl(_prompt_context: serde_json::Value) -> AppResult<serde_json::Value> {
    Err(AppError::NotImplemented(
        "generate_strategy_dsl: Phase C. Read key via secrets::keychain, call provider, \
         return raw response. Must NOT auto-approve or auto-queue (TODO.md).",
    ))
}

/// Server-side re-validation of a DSL using the SAME whitelist rules as the
/// frontend core validator. This command does not persist or queue anything;
/// the runner independently repeats the same validation at admission.
#[tauri::command]
pub fn validate_strategy_dsl(dsl: serde_json::Value) -> AppResult<serde_json::Value> {
    serde_json::to_value(alpha_factor_forge::discovery_core::dsl::validate_strategy_dsl(&dsl))
        .map_err(AppError::from)
}
