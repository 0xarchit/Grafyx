use std::fs;
use tempfile::tempdir;
use ed25519_dalek::{SigningKey, Signer};
use rand_core::OsRng;
use base64::prelude::*;
use grafyx::update::verify_signature_with_key;

// We mock the update module logic here to ensure it works with real ed25519 assets
#[test]
fn test_update_verification_flow() {
    let dir = tempdir().expect("Failed to create temp dir");
    let bin_path = dir.path().join("grafyx_new");
    
    // 1. Generate a keypair
    let signing_key = SigningKey::generate(&mut OsRng);
    let pub_key_b64 = BASE64_STANDARD.encode(signing_key.verifying_key().to_bytes());
    
    // 2. Create dummy binary
    let binary_content = b"v0.2.0 production binary content";
    fs::write(&bin_path, binary_content).unwrap();
    
    // 3. Sign it
    let signature = signing_key.sign(binary_content);
    let sig_bytes = signature.to_bytes();
    
    // 4. Verify using the production entrypoint logic
    verify_signature_with_key(&bin_path, &sig_bytes, &pub_key_b64)
        .expect("Signature verification should pass via production entrypoint");
}

#[test]
fn test_fails_on_tamper() {
    let dir = tempdir().expect("Failed to create temp dir");
    let bin_path = dir.path().join("grafyx_tampered");
    
    let signing_key = SigningKey::generate(&mut OsRng);
    let pub_key_b64 = BASE64_STANDARD.encode(signing_key.verifying_key().to_bytes());
    
    // 1. Write original content and sign it
    let binary_content = b"original content";
    fs::write(&bin_path, binary_content).unwrap();
    let original_on_disk = fs::read(&bin_path).unwrap();
    let signature = signing_key.sign(&original_on_disk);
    let sig_bytes = signature.to_bytes();
    
    // 2. Tamper with binary after signing
    fs::write(&bin_path, b"tampered content").unwrap();
    
    // 3. Verify using the production entrypoint logic - should fail
    let result = verify_signature_with_key(&bin_path, &sig_bytes, &pub_key_b64);
    assert!(result.is_err(), "Verification must fail on tampered content");
}
