use std::path::Path;
use anyhow::{Result, Context};
use ed25519_dalek::{Verifier, Signature, VerifyingKey};
use base64::prelude::*;

pub const PUBLIC_KEY_BASE64: &str = "clejACfR3BvqIUnmJARgypZrxI+aGVSJ91NbB2ymckM=";

pub fn verify_signature(binary_path: &Path, signature_bytes: &[u8]) -> Result<()> {
    let binary_data = std::fs::read(binary_path).context("Failed to read binary for verification")?;
    
    let key_bytes = BASE64_STANDARD.decode(PUBLIC_KEY_BASE64).context("Failed to decode public key")?;
    let verifying_key = VerifyingKey::from_bytes(key_bytes.as_slice().try_into().context("Invalid public key length")?)
        .context("Failed to initialize verifying key")?;
        
    let signature = Signature::from_bytes(signature_bytes.try_into().context("Invalid signature length")?);
    
    verifying_key.verify(&binary_data, &signature)
        .context("Signature verification failed! The binary may have been tampered with or is from an untrusted source.")?;
        
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{SigningKey, Signer};
    use rand_core::OsRng;

    #[test]
    fn test_verification_logic() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        
        let data = b"test binary content";
        let signature = signing_key.sign(data);
        
        verifying_key.verify(data, &signature).expect("Should verify same keys");
    }

    #[test]
    fn test_production_key_format() {
        let key_bytes = BASE64_STANDARD.decode(PUBLIC_KEY_BASE64).expect("Should decode");
        assert_eq!(key_bytes.len(), 32);
        VerifyingKey::from_bytes(key_bytes.as_slice().try_into().unwrap()).expect("Should init key");
    }
    #[test]
    fn test_verify_signature_fail_invalid_data() {
        use tempfile::NamedTempFile;
        use std::io::Write;
        
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"wrong data").unwrap();
        
        let sig = [0u8; 64];
        let result = verify_signature(file.path(), &sig);
        assert!(result.is_err());
    }
}
