use std::process::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_full_scan_smoke() {
    let dir = tempdir().expect("Failed to create temp dir");
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    
    fs::create_dir_all(&input_dir).unwrap();
    
    // Create a dummy python file
    let py_file = input_dir.join("main.py");
    fs::write(&py_file, "import os\nos.getcwd()").unwrap();
    
    // Build the binary path
    let mut binary_path = std::env::current_exe().expect("Failed to get current exe");
    binary_path.pop(); // Remove exe name
    if binary_path.ends_with("deps") {
        binary_path.pop();
    }
    let mut binary = binary_path.join("grafyx");
    if cfg!(windows) {
        binary.set_extension("exe");
    }
    
    let status = Command::new(binary)
        .arg("scan")
        .arg("--dirs")
        .arg(input_dir.to_str().unwrap())
        .arg("--output")
        .arg(output_dir.to_str().unwrap())
        .status()
        .expect("Failed to run binary");
        
    assert!(status.success());
    
    assert!(output_dir.join("grafyx.json").exists());
    assert!(output_dir.join("grafyx.db").exists());
    assert!(output_dir.join("index.html").exists());
}
