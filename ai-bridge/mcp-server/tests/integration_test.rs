use std::process::{Command, Stdio};
use std::io::Write;
use std::time::Duration;

#[test]
fn test_help_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_ai-bridge-mcp"))
        .arg("--help")
        .output()
        .expect("Failed to run --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ai-bridge-mcp"));
}

#[test]
fn test_run_starts_and_exits() {
    // Start the server (stdin EOF will cause clean MCP transport shutdown)
    let port = 19527u16;
    let mut child = Command::new(env!("CARGO_BIN_EXE_ai-bridge-mcp"))
        .arg("run")
        .arg("--ws-port")
        .arg(port.to_string())
        .arg("--log-file")
        .arg("/tmp/ai-bridge-integration-test.log")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn server");

    std::thread::sleep(Duration::from_millis(500));

    // Close stdin → MCP transport receives EOF → server exits cleanly
    drop(child.stdin.take());

    let status = child.wait().expect("Failed to wait on child");
    // Server may exit cleanly (stdin EOF) or need killing — any exit is fine
    eprintln!("Server exited with: {:?}", status);
}

#[test]
fn test_install_shows_config() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ai-bridge-mcp"))
        .arg("install")
        .arg("--token")
        .arg("test-token-123")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn install");

    // Send "n" to cancel
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(b"n\n").expect("Failed to write to stdin");

    let output = child.wait_with_output().expect("Failed to wait on child");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("AI_BRIDGE_MCP_TOKEN"));
    assert!(stdout.contains("test-token-123"));
}
