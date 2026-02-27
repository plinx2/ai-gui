use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::json;
use std::path::Path;
use tokio::fs;

use crate::agent::tool::Tool;

fn expand_path(path: &str) -> String {
    if path == "~" {
        return home_dir();
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        return format!("{}/{}", home_dir(), rest);
    }
    path.to_string()
}

fn home_dir() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string())
}

// ─── read_file ────────────────────────────────────────────────────────────────

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Returns text content by default, or base64-encoded bytes when encoding is set to \"base64\"."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file"
                },
                "encoding": {
                    "type": "string",
                    "enum": ["text", "base64"],
                    "description": "How to return the content. Default: \"text\""
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let path = match input["path"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"path\"".to_string(),
        };
        let encoding = input["encoding"].as_str().unwrap_or("text");

        match fs::read(&path).await {
            Ok(bytes) => {
                if encoding == "base64" {
                    BASE64.encode(&bytes)
                } else {
                    match String::from_utf8(bytes) {
                        Ok(text) => text,
                        Err(_) => "error: file is not valid UTF-8. Use encoding=\"base64\" to read binary files.".to_string(),
                    }
                }
            }
            Err(e) => format!("error: {e}"),
        }
    }
}

// ─── write_file ───────────────────────────────────────────────────────────────

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write text content to a file, creating it if it does not exist. Optionally creates parent directories."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file"
                },
                "content": {
                    "type": "string",
                    "description": "Text content to write"
                },
                "create_dirs": {
                    "type": "boolean",
                    "description": "Create parent directories if they do not exist (default: false)"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let path = match input["path"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"path\"".to_string(),
        };
        let content = match input["content"].as_str() {
            Some(c) => c.to_string(),
            None => return "error: missing required parameter \"content\"".to_string(),
        };
        let create_dirs = input["create_dirs"].as_bool().unwrap_or(false);

        if create_dirs {
            if let Some(parent) = Path::new(&path).parent() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    return format!("error creating directories: {e}");
                }
            }
        }

        match fs::write(&path, content.as_bytes()).await {
            Ok(_) => format!("ok: wrote {} bytes to {}", content.len(), path),
            Err(e) => format!("error: {e}"),
        }
    }
}

// ─── list_directory ───────────────────────────────────────────────────────────

pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List files and directories inside a directory. Supports recursive listing."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "List files recursively (default: false)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let path = match input["path"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"path\"".to_string(),
        };
        let recursive = input["recursive"].as_bool().unwrap_or(false);

        let mut entries: Vec<String> = Vec::new();
        if let Err(e) = collect_entries(Path::new(&path), &mut entries, recursive, 0).await {
            return format!("error: {e}");
        }
        if entries.is_empty() {
            "(empty directory)".to_string()
        } else {
            entries.join("\n")
        }
    }
}

async fn collect_entries(
    dir: &Path,
    out: &mut Vec<String>,
    recursive: bool,
    depth: usize,
) -> std::io::Result<()> {
    let mut read_dir = fs::read_dir(dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let meta = entry.metadata().await?;
        let name = entry.file_name().to_string_lossy().to_string();
        let prefix = "  ".repeat(depth);
        let kind = if meta.is_dir() { "DIR " } else { "FILE" };
        out.push(format!("{prefix}{kind}  {name}"));
        if recursive && meta.is_dir() {
            Box::pin(collect_entries(&entry.path(), out, true, depth + 1)).await?;
        }
    }
    Ok(())
}

// ─── search_in_files ──────────────────────────────────────────────────────────

pub struct SearchInFilesTool;

#[async_trait]
impl Tool for SearchInFilesTool {
    fn name(&self) -> &str {
        "search_in_files"
    }

    fn description(&self) -> &str {
        "Search for a text pattern inside files in a directory (like grep). Returns matching file paths and lines."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory (or file) to search in"
                },
                "pattern": {
                    "type": "string",
                    "description": "Text pattern to search for (case-insensitive)"
                },
                "file_glob": {
                    "type": "string",
                    "description": "Optional file extension filter, e.g. \".rs\" or \".ts\""
                }
            },
            "required": ["path", "pattern"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let path = match input["path"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"path\"".to_string(),
        };
        let pattern = match input["pattern"].as_str() {
            Some(p) => p.to_lowercase(),
            None => return "error: missing required parameter \"pattern\"".to_string(),
        };
        let file_glob = input["file_glob"].as_str().map(|s| s.to_string());

        let mut results: Vec<String> = Vec::new();
        if let Err(e) = search_files(Path::new(&path), &pattern, &file_glob, &mut results).await {
            return format!("error: {e}");
        }
        if results.is_empty() {
            format!("no matches found for \"{}\"", pattern)
        } else {
            results.join("\n")
        }
    }
}

async fn search_files(
    path: &Path,
    pattern: &str,
    file_glob: &Option<String>,
    results: &mut Vec<String>,
) -> std::io::Result<()> {
    let meta = fs::metadata(path).await?;
    if meta.is_file() {
        search_single_file(path, pattern, file_glob, results).await?;
    } else if meta.is_dir() {
        let mut read_dir = fs::read_dir(path).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let child_meta = entry.metadata().await?;
            let child_path = entry.path();
            if child_meta.is_dir() {
                Box::pin(search_files(&child_path, pattern, file_glob, results)).await?;
            } else {
                search_single_file(&child_path, pattern, file_glob, results).await?;
            }
        }
    }
    Ok(())
}

async fn search_single_file(
    path: &Path,
    pattern: &str,
    file_glob: &Option<String>,
    results: &mut Vec<String>,
) -> std::io::Result<()> {
    if let Some(ext) = file_glob {
        let name = path.to_string_lossy();
        if !name.ends_with(ext.as_str()) {
            return Ok(());
        }
    }
    let Ok(text) = fs::read_to_string(path).await else {
        return Ok(()); // skip binary files silently
    };
    for (i, line) in text.lines().enumerate() {
        if line.to_lowercase().contains(pattern) {
            results.push(format!("{}:{}: {}", path.display(), i + 1, line));
        }
    }
    Ok(())
}

// ─── move_file ────────────────────────────────────────────────────────────────

pub struct MoveFileTool;

#[async_trait]
impl Tool for MoveFileTool {
    fn name(&self) -> &str {
        "move_file"
    }

    fn description(&self) -> &str {
        "Move or rename a file or directory."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "from": {
                    "type": "string",
                    "description": "Source path"
                },
                "to": {
                    "type": "string",
                    "description": "Destination path"
                }
            },
            "required": ["from", "to"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let from = match input["from"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"from\"".to_string(),
        };
        let to = match input["to"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"to\"".to_string(),
        };
        match fs::rename(&from, &to).await {
            Ok(_) => format!("ok: moved {} → {}", from, to),
            Err(e) => format!("error: {e}"),
        }
    }
}

// ─── copy_file ────────────────────────────────────────────────────────────────

pub struct CopyFileTool;

#[async_trait]
impl Tool for CopyFileTool {
    fn name(&self) -> &str {
        "copy_file"
    }

    fn description(&self) -> &str {
        "Copy a file to a new location."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "from": {
                    "type": "string",
                    "description": "Source file path"
                },
                "to": {
                    "type": "string",
                    "description": "Destination file path"
                }
            },
            "required": ["from", "to"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let from = match input["from"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"from\"".to_string(),
        };
        let to = match input["to"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"to\"".to_string(),
        };
        match fs::copy(&from, &to).await {
            Ok(bytes) => format!("ok: copied {} bytes from {} to {}", bytes, from, to),
            Err(e) => format!("error: {e}"),
        }
    }
}

// ─── trash_file ───────────────────────────────────────────────────────────────

pub struct TrashFileTool;

#[async_trait]
impl Tool for TrashFileTool {
    fn name(&self) -> &str {
        "trash_file"
    }

    fn description(&self) -> &str {
        "Move a file or directory to the OS trash/recycle bin instead of permanently deleting it."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file or directory to move to trash"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let path = match input["path"].as_str() {
            Some(p) => expand_path(p),
            None => return "error: missing required parameter \"path\"".to_string(),
        };
        tokio::task::spawn_blocking(move || match trash::delete(&path) {
            Ok(_) => format!("ok: moved to trash: {}", path),
            Err(e) => format!("error: {e}"),
        })
        .await
        .unwrap_or_else(|e| format!("error: task panicked: {e}"))
    }
}
