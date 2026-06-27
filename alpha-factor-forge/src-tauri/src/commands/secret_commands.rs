// STUB — Phase C secrets. API keys live in the OS keychain, managed HERE.
// HARD RULES (never violate when implementing):
//   - Key is NEVER returned to the frontend.
//   - Key is NEVER written to localStorage or SQLite (plaintext or otherwise).
//   - Frontend may only learn whether a key EXISTS (status), and trigger a test.
// Local impl: use the `keyring` crate (add to Cargo.toml). Service name e.g.
//   "com.alphafactorforge.desktop", account = provider ("anthropic" | "openai").

use crate::error::{AppError, AppResult};

#[tauri::command]
pub fn save_ai_api_key(_provider: String, _key: String) -> AppResult<()> {
    // keyring::Entry::new(SERVICE, &provider)?.set_password(&key)?;
    Err(AppError::NotImplemented(
        "save_ai_api_key: store via keyring crate; never echo the key back (TODO.md).",
    ))
}

/// Returns whether a key is configured — NOT the key itself.
#[tauri::command]
pub fn get_ai_api_key_status(_provider: String) -> AppResult<bool> {
    Err(AppError::NotImplemented(
        "get_ai_api_key_status: keyring get_password().is_ok() -> bool (TODO.md).",
    ))
}

#[tauri::command]
pub fn delete_ai_api_key(_provider: String) -> AppResult<()> {
    Err(AppError::NotImplemented(
        "delete_ai_api_key: keyring delete_password() (TODO.md).",
    ))
}

/// Reads the key from keychain and pings the provider with a tiny request.
/// Returns ok/err only — no key, no full response leakage.
#[tauri::command]
pub fn test_ai_connection(_provider: String) -> AppResult<bool> {
    Err(AppError::NotImplemented(
        "test_ai_connection: keychain -> minimal provider ping -> bool (TODO.md).",
    ))
}
