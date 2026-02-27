use async_trait::async_trait;
use serde_json::json;
use tokio::process::Command;

use crate::agent::tool::Tool;

// ─── SSH config parser ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct SshHost {
    alias: String,
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
}

fn ssh_config_path() -> String {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    format!("{home}/.ssh/config")
}

fn parse_ssh_config(content: &str) -> Vec<SshHost> {
    let mut hosts: Vec<SshHost> = Vec::new();
    let mut current: Option<SshHost> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = match line.split_once(|c: char| c.is_whitespace()) {
            Some((k, v)) => (k.to_lowercase(), v.trim().to_string()),
            None => continue,
        };

        match key.as_str() {
            "host" => {
                if let Some(h) = current.take() {
                    // skip wildcard entries
                    if h.alias != "*" && !h.alias.contains('*') && !h.alias.contains('?') {
                        hosts.push(h);
                    }
                }
                current = Some(SshHost {
                    alias: value,
                    hostname: None,
                    user: None,
                    port: None,
                    identity_file: None,
                });
            }
            "hostname" => {
                if let Some(h) = current.as_mut() {
                    h.hostname = Some(value);
                }
            }
            "user" => {
                if let Some(h) = current.as_mut() {
                    h.user = Some(value);
                }
            }
            "port" => {
                if let Some(h) = current.as_mut() {
                    h.port = value.parse().ok();
                }
            }
            "identityfile" => {
                if let Some(h) = current.as_mut() {
                    h.identity_file = Some(value);
                }
            }
            _ => {}
        }
    }
    if let Some(h) = current {
        if h.alias != "*" && !h.alias.contains('*') && !h.alias.contains('?') {
            hosts.push(h);
        }
    }
    hosts
}

async fn read_ssh_config() -> Result<Vec<SshHost>, String> {
    let path = ssh_config_path();
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => Ok(parse_ssh_config(&content)),
        Err(e) => Err(format!("could not read {path}: {e}")),
    }
}

// ─── ssh_list_hosts ───────────────────────────────────────────────────────────

pub struct SshListHostsTool;

#[async_trait]
impl Tool for SshListHostsTool {
    fn name(&self) -> &str {
        "ssh_list_hosts"
    }

    fn description(&self) -> &str {
        "List SSH hosts defined in ~/.ssh/config. \
        IMPORTANT: After calling this tool you MUST call present_choices with the returned \
        host aliases as the choices array — never write the list as plain text in your reply. \
        This lets the user pick interactively."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: serde_json::Value) -> String {
        match read_ssh_config().await {
            Err(e) => format!("error: {e}"),
            Ok(hosts) if hosts.is_empty() => {
                "no hosts found in ~/.ssh/config (only wildcard Host * entries were skipped)".to_string()
            }
            Ok(hosts) => {
                // One line per host: "alias (hostname, user: xxx)"
                // These lines are ready to be passed directly as the choices array to present_choices.
                let lines: Vec<String> = hosts
                    .iter()
                    .map(|h| {
                        let mut meta = Vec::new();
                        if let Some(ref hn) = h.hostname {
                            meta.push(hn.clone());
                        }
                        if let Some(ref u) = h.user {
                            meta.push(format!("user: {u}"));
                        }
                        if let Some(p) = h.port {
                            meta.push(format!("port: {p}"));
                        }
                        if meta.is_empty() {
                            h.alias.clone()
                        } else {
                            format!("{} ({})", h.alias, meta.join(", "))
                        }
                    })
                    .collect();
                format!(
                    "Found {} host(s). Pass these as choices to present_choices:\n{}",
                    lines.len(),
                    lines.join("\n")
                )
            }
        }
    }
}

// ─── ssh_exec ─────────────────────────────────────────────────────────────────

pub struct SshExecTool;

#[async_trait]
impl Tool for SshExecTool {
    fn name(&self) -> &str {
        "ssh_exec"
    }

    fn description(&self) -> &str {
        "Execute a shell command on a remote host via SSH. The host must be defined in ~/.ssh/config. \
        If the target host is not yet determined, call ssh_list_hosts then present_choices first. \
        Uses key-based authentication only (no password prompts)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "description": "Host alias from ~/.ssh/config"
                },
                "command": {
                    "type": "string",
                    "description": "Shell command to run on the remote host"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Connection timeout in seconds (default: 15)"
                }
            },
            "required": ["host", "command"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let host = match input["host"].as_str() {
            Some(h) => h.to_string(),
            None => return "error: missing required parameter \"host\"".to_string(),
        };
        let command = match input["command"].as_str() {
            Some(c) => c.to_string(),
            None => return "error: missing required parameter \"command\"".to_string(),
        };
        let timeout = input["timeout_secs"].as_u64().unwrap_or(15);

        // Validate host exists in config
        match read_ssh_config().await {
            Err(e) => return format!("error: {e}"),
            Ok(hosts) => {
                if !hosts.iter().any(|h| h.alias == host) {
                    return format!(
                        "error: host \"{host}\" not found in ~/.ssh/config. Use ssh_list_hosts to see available hosts."
                    );
                }
            }
        }

        let output = Command::new("ssh")
            .args([
                "-o", "BatchMode=yes",
                "-o", &format!("ConnectTimeout={timeout}"),
                "-o", "StrictHostKeyChecking=accept-new",
                &host,
                &command,
            ])
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let code = out.status.code().unwrap_or(-1);
                let mut result = format!("exit code: {code}");
                if !stdout.is_empty() {
                    result.push('\n');
                    result.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    result.push_str("\n[stderr]\n");
                    result.push_str(&stderr);
                }
                result
            }
            Err(e) => format!("error: failed to spawn ssh: {e}"),
        }
    }
}

// ─── ssh_upload ───────────────────────────────────────────────────────────────

pub struct SshUploadTool;

#[async_trait]
impl Tool for SshUploadTool {
    fn name(&self) -> &str {
        "ssh_upload"
    }

    fn description(&self) -> &str {
        "Upload a local file to a remote host via SCP. The host must be defined in ~/.ssh/config."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "description": "Host alias from ~/.ssh/config"
                },
                "local_path": {
                    "type": "string",
                    "description": "Local file path to upload"
                },
                "remote_path": {
                    "type": "string",
                    "description": "Destination path on the remote host (e.g. \"/home/user/file.txt\")"
                }
            },
            "required": ["host", "local_path", "remote_path"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let host = match input["host"].as_str() {
            Some(h) => h.to_string(),
            None => return "error: missing required parameter \"host\"".to_string(),
        };
        let local_path = match input["local_path"].as_str() {
            Some(p) => p.to_string(),
            None => return "error: missing required parameter \"local_path\"".to_string(),
        };
        let remote_path = match input["remote_path"].as_str() {
            Some(p) => p.to_string(),
            None => return "error: missing required parameter \"remote_path\"".to_string(),
        };

        let dest = format!("{host}:{remote_path}");
        let output = Command::new("scp")
            .args(["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new", &local_path, &dest])
            .output()
            .await;

        match output {
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                if out.status.success() {
                    format!("ok: uploaded {local_path} → {dest}")
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    format!("error (exit {code}): {stderr}")
                }
            }
            Err(e) => format!("error: failed to spawn scp: {e}"),
        }
    }
}

// ─── ssh_download ─────────────────────────────────────────────────────────────

pub struct SshDownloadTool;

#[async_trait]
impl Tool for SshDownloadTool {
    fn name(&self) -> &str {
        "ssh_download"
    }

    fn description(&self) -> &str {
        "Download a file from a remote host via SCP. The host must be defined in ~/.ssh/config."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "description": "Host alias from ~/.ssh/config"
                },
                "remote_path": {
                    "type": "string",
                    "description": "File path on the remote host"
                },
                "local_path": {
                    "type": "string",
                    "description": "Local destination path"
                }
            },
            "required": ["host", "remote_path", "local_path"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let host = match input["host"].as_str() {
            Some(h) => h.to_string(),
            None => return "error: missing required parameter \"host\"".to_string(),
        };
        let remote_path = match input["remote_path"].as_str() {
            Some(p) => p.to_string(),
            None => return "error: missing required parameter \"remote_path\"".to_string(),
        };
        let local_path = match input["local_path"].as_str() {
            Some(p) => p.to_string(),
            None => return "error: missing required parameter \"local_path\"".to_string(),
        };

        let src = format!("{host}:{remote_path}");
        let output = Command::new("scp")
            .args(["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new", &src, &local_path])
            .output()
            .await;

        match output {
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                if out.status.success() {
                    format!("ok: downloaded {src} → {local_path}")
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    format!("error (exit {code}): {stderr}")
                }
            }
            Err(e) => format!("error: failed to spawn scp: {e}"),
        }
    }
}
