use async_trait::async_trait;

use crate::agent::tool::Tool;

pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "run_shell_command"
    }

    fn description(&self) -> &str {
        "Executes a shell command on the user's machine and returns stdout/stderr output"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let command = match input.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd.to_string(),
            None => return "Error: 'command' field is required".to_string(),
        };

        #[cfg(target_os = "windows")]
        let output = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &command])
            .output()
            .await;

        #[cfg(not(target_os = "windows"))]
        let output = tokio::process::Command::new("sh")
            .args(["-c", &command])
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let exit_code = out.status.code().unwrap_or(-1);

                if stderr.is_empty() {
                    format!("exit code: {}\n{}", exit_code, stdout)
                } else {
                    format!(
                        "exit code: {}\nstdout:\n{}\nstderr:\n{}",
                        exit_code, stdout, stderr
                    )
                }
            }
            Err(e) => format!("Failed to execute command: {}", e),
        }
    }
}
