use anyhow::Result;
use std::process::Command;

pub struct TestContext;

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TestContext {
    pub fn new() -> Self {
        TestContext
    }

    pub fn run_probe(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("cargo")
            .args(["run", "--"])
            .args(args)
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Command failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
