use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use tokio::sync::Mutex;

use crate::agent::tool::Tool;

/// Percent-encode a search query for use in a URL query string.
/// Spaces → '+', non-ASCII and special chars → %XX.
fn url_encode_query(s: &str) -> String {
    let mut out = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Shared browser state
// ---------------------------------------------------------------------------

pub type SharedBrowser = Arc<Mutex<Option<BrowserState>>>;

pub struct BrowserState {
    _browser: Browser, // keep alive so Chrome doesn't exit
    page: chromiumoxide::Page,
    _handler: tokio::task::JoinHandle<()>,
}

fn chrome_executable() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        for path in &candidates {
            if std::path::Path::new(path).exists() {
                return std::path::PathBuf::from(path);
            }
        }
        std::path::PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe")
    }
    #[cfg(target_os = "macos")]
    {
        std::path::PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        )
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::path::PathBuf::from("/usr/bin/google-chrome")
    }
}

async fn launch_browser() -> Result<BrowserState, String> {
    let config = BrowserConfig::builder()
        .chrome_executable(chrome_executable())
        .with_head()
        // Hide automation flags so sites don't detect the bot
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .build()
        .map_err(|e| format!("Browser config error: {e}"))?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("Failed to launch Chrome: {e}"))?;

    let handler_task = tokio::spawn(async move {
        while let Some(_) = handler.next().await {}
    });

    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("Failed to create browser page: {e}"))?;

    Ok(BrowserState {
        _browser: browser,
        page,
        _handler: handler_task,
    })
}

// ---------------------------------------------------------------------------
// Factory — creates all browser tools sharing one browser instance
// ---------------------------------------------------------------------------

pub fn create_browser_tools() -> (SharedBrowser, Vec<Box<dyn Tool>>) {
    let shared: SharedBrowser = Arc::new(Mutex::new(None));
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(BrowserSearchTool { shared: Arc::clone(&shared) }),
        Box::new(BrowserNavigateTool { shared: Arc::clone(&shared) }),
        Box::new(BrowserGetUrlTool { shared: Arc::clone(&shared) }),
        Box::new(BrowserGetTextTool { shared: Arc::clone(&shared) }),
        Box::new(BrowserGetLinksTool { shared: Arc::clone(&shared) }),
        Box::new(BrowserClickTool { shared: Arc::clone(&shared) }),
        Box::new(BrowserTypeTool { shared: Arc::clone(&shared) }),
        Box::new(BrowserScreenshotTool { shared: Arc::clone(&shared) }),
        Box::new(BrowserCloseTool { shared: Arc::clone(&shared) }),
    ];
    (shared, tools)
}

// ---------------------------------------------------------------------------
// browser_navigate
// ---------------------------------------------------------------------------

pub struct BrowserNavigateTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserNavigateTool {
    fn name(&self) -> &str {
        "browser_navigate"
    }

    fn description(&self) -> &str {
        "Opens a URL in Chrome. Launches Chrome automatically if it is not already open. \
         Returns the page title once navigation completes. \
         Always include the full URL with http:// or https://."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Full URL to navigate to (e.g. https://www.google.com)"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let url = match input.get("url").and_then(|v| v.as_str()) {
            Some(u) => u.to_string(),
            None => return "Error: 'url' is required".to_string(),
        };

        let mut guard = self.shared.lock().await;
        if guard.is_none() {
            match launch_browser().await {
                Ok(state) => *guard = Some(state),
                Err(e) => return e,
            }
        }

        let state = guard.as_ref().unwrap();
        match state.page.goto(&url).await {
            Ok(_) => {
                // Give the page a moment to finish loading
                let _ = tokio::time::timeout(
                    Duration::from_secs(5),
                    state.page.wait_for_navigation(),
                )
                .await;

                // Mask navigator.webdriver to reduce bot detection
                let _ = state
                    .page
                    .evaluate(
                        "Object.defineProperty(navigator,'webdriver',{get:()=>undefined})",
                    )
                    .await;

                let title = state
                    .page
                    .get_title()
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| url.clone());

                format!("Navigated to: {url}\nPage title: {title}")
            }
            Err(e) => format!("Navigation error: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_get_text
// ---------------------------------------------------------------------------

pub struct BrowserGetTextTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserGetTextTool {
    fn name(&self) -> &str {
        "browser_get_text"
    }

    fn description(&self) -> &str {
        "Gets the visible text content of the current browser page. \
         Use this to read page content, search results, listings, or any text on screen. \
         Returns up to 8000 characters by default."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "max_chars": {
                    "type": "integer",
                    "description": "Maximum characters to return (default: 8000)"
                }
            }
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let max_chars = input
            .get("max_chars")
            .and_then(|v| v.as_u64())
            .unwrap_or(8000) as usize;

        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open. Use browser_navigate to open a URL first.".to_string();
        };

        match state
            .page
            .evaluate("document.body ? document.body.innerText : ''")
            .await
        {
            Ok(result) => {
                let text = result.into_value::<String>().unwrap_or_default();
                if text.len() > max_chars {
                    format!("{}…[truncated at {max_chars} chars]", &text[..max_chars])
                } else {
                    text
                }
            }
            Err(e) => format!("Error getting page text: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_get_links
// ---------------------------------------------------------------------------

pub struct BrowserGetLinksTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserGetLinksTool {
    fn name(&self) -> &str {
        "browser_get_links"
    }

    fn description(&self) -> &str {
        "Gets all links on the current page as a JSON array of {text, href} objects. \
         Useful for discovering navigation options, search results, or URLs to follow."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "max_links": {
                    "type": "integer",
                    "description": "Maximum number of links to return (default: 30)"
                }
            }
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let max_links = input
            .get("max_links")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };

        let js = format!(
            r#"JSON.stringify(
                Array.from(document.querySelectorAll('a[href]'))
                    .filter(a => a.innerText.trim().length > 0 && a.href.startsWith('http'))
                    .slice(0, {max_links})
                    .map(a => ({{text: a.innerText.trim().replace(/\s+/g, ' '), href: a.href}}))
            )"#
        );

        match state.page.evaluate(js.as_str()).await {
            Ok(result) => result.into_value::<String>().unwrap_or_default(),
            Err(e) => format!("Error getting links: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_click
// ---------------------------------------------------------------------------

pub struct BrowserClickTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserClickTool {
    fn name(&self) -> &str {
        "browser_click"
    }

    fn description(&self) -> &str {
        "Clicks an element on the current page identified by a CSS selector. \
         Examples: 'button.search-submit', 'a[href*=\"example\"]', 'input[type=\"submit\"]'."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "CSS selector of the element to click"
                }
            },
            "required": ["selector"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let selector = match input.get("selector").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return "Error: 'selector' is required".to_string(),
        };

        let result = {
            let guard = self.shared.lock().await;
            let Some(state) = guard.as_ref() else {
                return "Browser not open.".to_string();
            };
            match state.page.find_element(&selector).await {
                Ok(elem) => match elem.click().await {
                    Ok(_) => {
                        // Wait for navigation that may be triggered by the click (up to 4 s)
                        let navigated = tokio::time::timeout(
                            Duration::from_secs(4),
                            state.page.wait_for_navigation(),
                        )
                        .await
                        .is_ok();
                        Ok(navigated)
                    }
                    Err(e) => Err(format!("Click error on '{selector}': {e}")),
                },
                Err(e) => Err(format!("Element not found '{selector}': {e}")),
            }
        };

        match result {
            Ok(navigated) => {
                if navigated {
                    format!("Clicked '{selector}' — page navigated")
                } else {
                    format!("Clicked '{selector}' — no navigation detected")
                }
            }
            Err(e) => e,
        }
    }
}

// ---------------------------------------------------------------------------
// browser_type
// ---------------------------------------------------------------------------

pub struct BrowserTypeTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserTypeTool {
    fn name(&self) -> &str {
        "browser_type"
    }

    fn description(&self) -> &str {
        "Types text into an input element. Clears any existing value first. \
         Set submit:true to press Enter after typing (useful for search boxes)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "CSS selector of the input/textarea element"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type into the element"
                },
                "submit": {
                    "type": "boolean",
                    "description": "Press Enter after typing to submit (default: false)"
                }
            },
            "required": ["selector", "text"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let selector = match input.get("selector").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return "Error: 'selector' is required".to_string(),
        };
        let text = match input.get("text").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return "Error: 'text' is required".to_string(),
        };
        let submit = input
            .get("submit")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let result = {
            let guard = self.shared.lock().await;
            let Some(state) = guard.as_ref() else {
                return "Browser not open.".to_string();
            };

            // Clear the existing field value via JS
            let safe_selector = selector.replace('\'', "\\'");
            let clear_js = format!(
                "(function(){{ var el=document.querySelector('{safe_selector}'); \
                 if(el){{el.value='';el.dispatchEvent(new Event('input',{{bubbles:true}}));return true;}} \
                 return false; }})()"
            );
            let _ = state.page.evaluate(clear_js.as_str()).await;

            match state.page.find_element(&selector).await {
                Ok(elem) => match elem.type_str(&text).await {
                    Ok(_) => {
                        if submit {
                            // 1. Keyboard events (for event-listener driven sites)
                            // 2. form.requestSubmit() (correct browser-like submit, triggers validation)
                            // 3. form.submit() fallback
                            let submit_js = format!(
                                r#"(function(){{
                                    var el=document.querySelector('{safe_selector}');
                                    if(!el)return;
                                    ['keydown','keypress','keyup'].forEach(function(t){{
                                        el.dispatchEvent(new KeyboardEvent(t,{{
                                            key:'Enter',code:'Enter',keyCode:13,
                                            which:13,bubbles:true,cancelable:true
                                        }}));
                                    }});
                                    if(el.form){{
                                        try{{el.form.requestSubmit();}}
                                        catch(e){{try{{el.form.submit();}}catch(e2){{}}}}
                                    }}
                                }})()"#
                            );
                            let _ = state.page.evaluate(submit_js.as_str()).await;
                            // Wait for the navigation that the submit triggers
                            let _ = tokio::time::timeout(
                                Duration::from_secs(6),
                                state.page.wait_for_navigation(),
                            )
                            .await;
                            Ok(true)
                        } else {
                            Ok(false)
                        }
                    }
                    Err(e) => Err(format!("Type error on '{selector}': {e}")),
                },
                Err(e) => Err(format!("Element not found '{selector}': {e}")),
            }
        };

        match result {
            Ok(submitted) => {
                if submitted {
                    format!("Typed '{text}' into '{selector}' and submitted")
                } else {
                    format!("Typed '{text}' into '{selector}'")
                }
            }
            Err(e) => e,
        }
    }
}

// ---------------------------------------------------------------------------
// browser_screenshot
// ---------------------------------------------------------------------------

pub struct BrowserScreenshotTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn name(&self) -> &str {
        "browser_screenshot"
    }

    fn description(&self) -> &str {
        "Takes a screenshot of the current browser page and returns it as a base64 PNG image. \
         Use this when you need to visually inspect the page layout or verify content."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: serde_json::Value) -> String {
        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };

        match state
            .page
            .screenshot(ScreenshotParams::builder().build())
            .await
        {
            Ok(bytes) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                // Prefixed so gemini.rs can attach this as an inlineData image part
                format!("SCREENSHOT:image/png:{b64}")
            }
            Err(e) => format!("Screenshot error: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_close
// ---------------------------------------------------------------------------

pub struct BrowserCloseTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserCloseTool {
    fn name(&self) -> &str {
        "browser_close"
    }

    fn description(&self) -> &str {
        "Closes the Chrome browser window and frees all browser resources."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: serde_json::Value) -> String {
        let mut guard = self.shared.lock().await;
        if guard.is_none() {
            return "Browser is not currently open.".to_string();
        }
        *guard = None; // drops BrowserState → Chrome exits
        "Browser closed.".to_string()
    }
}

// ---------------------------------------------------------------------------
// browser_get_url
// ---------------------------------------------------------------------------

pub struct BrowserGetUrlTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserGetUrlTool {
    fn name(&self) -> &str {
        "browser_get_url"
    }

    fn description(&self) -> &str {
        "Returns the current URL of the browser page. \
         Useful for confirming navigation succeeded or determining where you are."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: serde_json::Value) -> String {
        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open.".to_string();
        };
        match state.page.url().await {
            Ok(Some(url)) => url,
            Ok(None) => "about:blank".to_string(),
            Err(e) => format!("Error getting URL: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_search  (navigates directly to Google search URL — most reliable)
// ---------------------------------------------------------------------------

pub struct BrowserSearchTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserSearchTool {
    fn name(&self) -> &str {
        "browser_search"
    }

    fn description(&self) -> &str {
        "Search the web by navigating directly to Google search results. \
         This is the PREFERRED way to search — much more reliable than typing in the \
         search box. Use this whenever you need to search for something."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query (Japanese or English)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return "Error: 'query' is required".to_string(),
        };

        let url = format!(
            "https://www.google.com/search?q={}&hl=ja",
            url_encode_query(&query)
        );

        let mut guard = self.shared.lock().await;
        if guard.is_none() {
            match launch_browser().await {
                Ok(state) => *guard = Some(state),
                Err(e) => return e,
            }
        }

        let state = guard.as_ref().unwrap();
        match state.page.goto(&url).await {
            Ok(_) => {
                let _ = tokio::time::timeout(
                    Duration::from_secs(5),
                    state.page.wait_for_navigation(),
                )
                .await;
                let _ = state
                    .page
                    .evaluate(
                        "Object.defineProperty(navigator,'webdriver',{get:()=>undefined})",
                    )
                    .await;
                let title = state
                    .page
                    .get_title()
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                format!("Searched Google for: '{query}'\nPage title: {title}")
            }
            Err(e) => format!("Search error: {e}"),
        }
    }
}
