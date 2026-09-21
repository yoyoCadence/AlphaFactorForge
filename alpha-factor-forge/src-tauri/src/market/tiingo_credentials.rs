//! P09 credential boundary. Only the Rust host reads Windows Credential Manager.
//! Provision the generic credential through the OS UI; no secret CLI argument,
//! environment fallback, frontend command, debug formatter or plaintext file.
pub const TARGET: &str = "com.alphafactorforge.desktop/tiingo";

pub struct TiingoToken(String);
impl TiingoToken {
    pub fn new(value: String) -> Result<Self, &'static str> {
        if value.is_empty()
            || value.len() > 512
            || !value.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err("credential_invalid");
        }
        Ok(Self(value))
    }
    pub(super) fn header(&self) -> String {
        format!("Token {}", self.0)
    }
}

#[cfg(windows)]
pub fn read() -> Result<TiingoToken, &'static str> {
    use windows_sys::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };
    let target: Vec<u16> = TARGET.encode_utf16().chain(Some(0)).collect();
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: NUL-terminated target, valid out pointer; successful buffers are
    // owned by Windows and freed exactly once after copying the password.
    unsafe {
        if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) == 0 {
            return Err(if windows_sys::Win32::Foundation::GetLastError() == 1168 {
                "credential_missing"
            } else {
                "credential_unavailable"
            });
        }
        let size = (*credential).CredentialBlobSize as usize;
        let result = if size == 0
            || size > 1024
            || !size.is_multiple_of(2)
            || (*credential).CredentialBlob.is_null()
        {
            Err("credential_invalid")
        } else {
            let bytes = std::slice::from_raw_parts((*credential).CredentialBlob, size);
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            String::from_utf16(&units)
                .map_err(|_| "credential_invalid")
                .and_then(TiingoToken::new)
        };
        CredFree(credential.cast());
        result
    }
}

#[cfg(not(windows))]
pub fn read() -> Result<TiingoToken, &'static str> {
    Err("credential_platform_unsupported")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_credentials_never_become_headers_or_error_values() {
        for value in [
            "",
            "secret\r\nx: injected",
            "Token secret",
            "secret?token=x",
        ] {
            assert!(matches!(
                TiingoToken::new(value.into()),
                Err("credential_invalid")
            ));
        }
        assert_eq!(
            TiingoToken::new("test123".into()).unwrap().header(),
            "Token test123"
        );
    }
}
