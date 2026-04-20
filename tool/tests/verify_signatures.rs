use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use ed25519_dalek::{SigningKey, Signer, Verifier, Signature, VerifyingKey};
use rand_core::OsRng;

// We mock the update module logic here to ensure it works with real ed25519 assets
#[test]
fn test_update_verification_flow() {
    let dir = tempdir().expect("Failed to create temp dir");
    let bin_path = dir.path().join("grafyx_new");
    let sig_path = dir.path().join("grafyx_new.sig");
    
    // 1. Generate a keypair
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    
    // 2. Create dummy binary
    let binary_content = b"v0.2.0 production binary content";
    fs::write(&bin_path, binary_content).unwrap();
    
    // 3. Sign it
    let signature = signing_key.sign(binary_content);
    let sig_bytes = signature.to_bytes();
    fs::write(&sig_path, sig_bytes).unwrap();
    
    // 4. Verify using the logic from update.rs
    let read_binary = fs::read(&bin_path).unwrap();
    let read_sig = fs::read(&sig_path).unwrap();
    
    let signature_to_verify = Signature::from_bytes(read_sig.as_slice().try_into().unwrap());
    verifying_key.verify(&read_binary, &signature_to_verify).expect("Signature verification should pass");
}

#[test]
fn test_fails_on_tamper() {
    let dir = tempdir().expect("Failed to create temp dir");
    let bin_path = dir.path().join("grafyx_tampered");
    
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    
    let binary_content = b"original content";
    let signature = signing_key.sign(binary_content);
    
    // Tamper with binary
    fs::write(&bin_path, b"tampered content").unwrap();
    
    let read_binary = fs::read(&bin_path).unwrap();
    assert!(verifying_key.verify(&read_binary, &signature).is_err());
}
