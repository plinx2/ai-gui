use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;

use crate::agent::tool::Tool;

pub struct HttpRequestTool {
    client: reqwest::Client,
}

impl HttpRequestTool {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for HttpRequestTool {
    fn name(&self) -> &str {
        "http_request"
    }

    fn description(&self) -> &str {
        "Make an HTTP request (GET, POST, PUT, PATCH, DELETE). Returns status code, response headers, and body."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Full URL to request"
                },
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "PATCH", "DELETE"],
                    "description": "HTTP method (default: \"GET\")"
                },
                "headers": {
                    "type": "object",
                    "description": "Optional request headers as key-value pairs"
                },
                "body": {
                    "type": "string",
                    "description": "Optional request body (for POST/PUT/PATCH)"
                },
                "content_type": {
                    "type": "string",
                    "description": "Content-Type header shorthand, e.g. \"application/json\""
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let url = match input["url"].as_str() {
            Some(u) => u.to_string(),
            None => return "error: missing required parameter \"url\"".to_string(),
        };
        let method = input["method"].as_str().unwrap_or("GET").to_uppercase();

        let req_method = match method.as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            "DELETE" => reqwest::Method::DELETE,
            other => return format!("error: unsupported method \"{other}\""),
        };

        let mut builder = self.client.request(req_method, &url);

        // Apply headers from the object
        if let Some(headers_map) = input["headers"].as_object() {
            for (key, value) in headers_map {
                if let Some(val_str) = value.as_str() {
                    builder = builder.header(key.as_str(), val_str);
                }
            }
        }

        // Content-Type shorthand
        if let Some(ct) = input["content_type"].as_str() {
            builder = builder.header("Content-Type", ct);
        }

        // Body
        if let Some(body) = input["body"].as_str() {
            builder = builder.body(body.to_string());
        }

        match builder.send().await {
            Ok(response) => {
                let status = response.status();
                // Collect select response headers
                let interesting_headers: HashMap<String, String> = response
                    .headers()
                    .iter()
                    .filter(|(k, _)| {
                        matches!(
                            k.as_str(),
                            "content-type" | "content-length" | "location" | "x-request-id"
                        )
                    })
                    .map(|(k, v)| {
                        (
                            k.to_string(),
                            v.to_str().unwrap_or("(non-utf8)").to_string(),
                        )
                    })
                    .collect();

                let body_text = response.text().await.unwrap_or_else(|_| "(unreadable body)".to_string());

                let header_lines: String = interesting_headers
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join("\n");

                format!("status: {}\n{}\n\n{}", status.as_u16(), header_lines, body_text)
            }
            Err(e) => format!("error: {e}"),
        }
    }
}
