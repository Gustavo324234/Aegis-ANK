use base64::prelude::*;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CitadelError {
    #[error("Missing AEGIS_ROOT_KEY environment variable")]
    MissingRootKey,
    #[error("Cryptographic error: {0}")]
    CryptoError(String),
}

#[derive(Debug, Clone)]
pub struct SafeIdentity {
    pub private_id: String,
    pub public_id: String,
}

/// Genera un alias determinista para el tenant_id usando HMAC-SHA256.
/// El resultado es URL-Safe Base64 (sin padding).
pub fn generate_public_tenant_id(raw_id: &str, root_key: &[u8]) -> Result<String, CitadelError> {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(root_key)
        .map_err(|e| CitadelError::CryptoError(e.to_string()))?;

    mac.update(raw_id.as_bytes());
    let result = mac.finalize();
    let bytes = result.into_bytes();

    Ok(BASE64_URL_SAFE_NO_PAD.encode(bytes))
}

/// Sanitiza un mensaje reemplazando el ID privado por el público.
pub fn sanitize_error(message: &str, private_id: &str, public_id: &str) -> String {
    message.replace(private_id, public_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_id_obfuscation() {
        let root_key = b"Aegis_Master_Secret";
        let tenant_id = "tenant-42";

        let hash1 = generate_public_tenant_id(tenant_id, root_key).expect("Should generate hash");
        let hash2 =
            generate_public_tenant_id(tenant_id, root_key).expect("Should be deterministic");

        assert_eq!(hash1, hash2);
        assert!(!hash1.contains(tenant_id));
        assert!(!hash1.is_empty());
    }

    #[test]
    fn test_grpc_error_leakage() {
        let private = "tenant-leak-007";
        let public = "REDACTED-HASH-XYZ";
        let msg = format!(
            "Critical error: database /data/{}/main.db is locked",
            private
        );

        let sanitized = sanitize_error(&msg, private, public);

        assert!(!sanitized.contains(private));
        assert!(sanitized.contains(public));
    }
}
