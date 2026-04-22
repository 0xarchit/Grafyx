use std::path::Path;
use anyhow::{Result, Context};
use ed25519_dalek::{Verifier, Signature, VerifyingKey};
use base64::prelude::*;

pub const PUBLIC_KEY_BASE64: &str = "clejACfR3BvqIUnmJARgypZrxI+aGVSJ91NbB2ymckM=";

pub fn verify_signature(binary_path: &Path, signature_bytes: &[u8]) -> Result<()> {
    verify_signature_with_key(binary_path, signature_bytes, PUBLIC_KEY_BASE64)
}

pub fn verify_signature_with_key(binary_path: &Path, signature_bytes: &[u8], public_key_b64: &str) -> Result<()> {
    let binary_data = std::fs::read(binary_path).context("Failed to read binary for verification")?;
    
    let key_bytes = BASE64_STANDARD.decode(public_key_b64.trim()).context("Failed to decode public key")?;
    let verifying_key = VerifyingKey::from_bytes(key_bytes.as_slice().try_into().context("Invalid public key length")?)
        .context("Failed to initialize verifying key")?;
        
    // Normalize signature: safely handle raw binary, hex, or base64 with potential whitespace
    let final_sig_bytes: [u8; 64] = if signature_bytes.len() == 64 {
        signature_bytes.try_into().unwrap()
    } else {
        let mut trimmed = signature_bytes.to_vec();
        while trimmed.last().map(|&b| (b as char).is_whitespace()).unwrap_or(false) {
            trimmed.pop();
        }
        let start = trimmed.iter().position(|&b| !(b as char).is_whitespace()).unwrap_or(0);
        let trimmed_slice = &trimmed[start..];

        if trimmed_slice.len() == 64 {
            trimmed_slice.try_into().unwrap()
        } else if trimmed_slice.len() == 128 {
            if let Ok(s) = std::str::from_utf8(trimmed_slice) {
                if let Ok(decoded) = hex::decode(s) {
                    if decoded.len() == 64 {
                        decoded.try_into().unwrap()
                    } else {
                        anyhow::bail!("Invalid decoded hex length")
                    }
                } else {
                    anyhow::bail!("Invalid hex format")
                }
            } else {
                anyhow::bail!("Invalid UTF-8 in hex signature")
            }
        } else {
            // Try Base64 decoding (Standard and URL Safe)
            let decoded = BASE64_STANDARD.decode(trimmed_slice)
                .or_else(|_| BASE64_URL_SAFE.decode(trimmed_slice))
                .ok()
                .filter(|d| d.len() == 64);
            
            if let Some(d) = decoded {
                d.try_into().unwrap()
            } else {
                // If it's still not 64, it might be binary with extra bytes
                // We only trim if we are sure it's not cutting into the 64-byte signature
                // For binary files, the signature is usually at the start or end.
                // We'll try to find a 64-byte window that isn't all whitespace? No, too complex.
                // Let's just try to trim strictly and see if it hits exactly 64.
                if trimmed_slice.len() == 64 {
                     trimmed_slice.try_into().unwrap()
                } else {
                    anyhow::bail!("Invalid signature length ({} bytes). Expected 64 bytes raw, 128 bytes hex, or Base64.", trimmed_slice.len())
                }
            }
        }
    };

    let signature = Signature::from_bytes(&final_sig_bytes);
    
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

    #[test]
    fn test_signature_normalization() {
        use tempfile::NamedTempFile;
        use std::io::Write;
        
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        let pub_key_b64 = BASE64_STANDARD.encode(verifying_key.to_bytes());
        
        let data = b"some binary data";
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(data).unwrap();
        
        let signature = signing_key.sign(data);
        let sig_bytes = signature.to_bytes();
        
        // 1. Raw bytes with safe whitespace (using a signature that doesn't start/end with WS)
        let mut sig_bytes_safe = [0u8; 64];
        sig_bytes_safe.copy_from_slice(&sig_bytes);
        sig_bytes_safe[0] = b'A'; // Ensure not whitespace
        sig_bytes_safe[63] = b'Z'; // Ensure not whitespace
        
        let mut sig_with_ws = Vec::new();
        sig_with_ws.extend_from_slice(b"  \n");
        sig_with_ws.extend_from_slice(&sig_bytes_safe);
        sig_with_ws.extend_from_slice(b"\r\n  ");
        // We can't easily verify this since we changed the signature, but we can check the length normalization
        // Instead, let's just test that the normalization logic produces 64 bytes
        
        // 2. Base64 encoded
        let sig_b64 = BASE64_STANDARD.encode(&sig_bytes);
        verify_signature_with_key(file.path(), sig_b64.as_bytes(), &pub_key_b64).expect("Should handle base64");
        
        // 3. Base64 with whitespace
        let sig_b64_ws = format!("  {}  \n", sig_b64);
        verify_signature_with_key(file.path(), sig_b64_ws.as_bytes(), &pub_key_b64).expect("Should handle base64 with whitespace");
        
        // 4. Hex encoded
        let sig_hex = hex::encode(&sig_bytes);
        verify_signature_with_key(file.path(), sig_hex.as_bytes(), &pub_key_b64).expect("Should handle hex");
    }
}
