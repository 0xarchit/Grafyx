use std::env;
use std::fs;
use std::path::Path;
use ed25519_dalek::{SigningKey, Signer};
use base64::prelude::*;
use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: sign <file_path>");
        std::process::exit(1);
    }
    
    let file_path = Path::new(&args[1]);
    let priv_key_base64 = env::var("GRAFYX_RELEASE_SIGNING_KEY")
        .context("Environment variable GRAFYX_RELEASE_SIGNING_KEY not set")?;
        
    let priv_key_bytes = BASE64_STANDARD.decode(priv_key_base64.trim())
        .context("Failed to decode secret signing key from Base64")?;
        
    let signing_key = SigningKey::from_bytes(
        priv_key_bytes.as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid private key length: expected 32 bytes"))?
    );
    
    if !file_path.exists() {
        bail!("File not found: {:?}", file_path);
    }
    
    println!("Signing asset: {:?}", file_path);
    let data = fs::read(file_path).context("Failed to read asset file")?;
    let signature = signing_key.sign(&data);
    
    let sig_path = format!("{}.sig", args[1]);
    fs::write(&sig_path, signature.to_bytes())
        .context("Failed to write signature file")?;
    
    println!("Successfully signed! Generated: {}", sig_path);
    Ok(())
}
