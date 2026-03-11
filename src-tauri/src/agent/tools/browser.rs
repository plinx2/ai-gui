use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use futures::future::BoxFuture;
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
    browser: Browser,
    active_page: Option<chromiumoxide::Page>,
    _handler: tokio::task::JoinHandle<()>,
}

impl BrowserState {
    /// Opens `url` in a new tab and sets it as the active page.
    async fn open(&mut self, url: &str) -> Result<String, String> {
        let page = self
            .browser
            .new_page(url)
            .await
            .map_err(|e| format!("Failed to open tab: {e}"))?;
        page.activate()
            .await
            .map_err(|e| format!("Failed to activate tab: {e}"))?;
        let _ = tokio::time::timeout(Duration::from_secs(5), page.wait_for_navigation()).await;
        let _ = page
            .evaluate("Object.defineProperty(navigator,'webdriver',{get:()=>undefined})")
            .await;
        let title = page
            .get_title()
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| url.to_string());
        self.active_page = Some(page);
        Ok(title)
    }

    /// Closes the current active tab and switches to another existing tab,
    /// or creates a blank tab if none remain.
    async fn close_page(&mut self) -> Result<(), String> {
        if let Some(page) = self.active_page.take() {
            let _ = page.close().await;
            let pages = self
                .browser
                .pages()
                .await
                .map_err(|e| format!("Failed to list pages: {e}"))?;
            if let Some(next) = pages.first() {
                let _ = next.activate().await;
                self.active_page = Some(next.clone());
            } else {
                let blank = self
                    .browser
                    .new_page("about:blank")
                    .await
                    .map_err(|e| format!("Failed to create blank page: {e}"))?;
                self.active_page = Some(blank);
            }
        }
        Ok(())
    }

    /// Returns a JSON list of all open tabs with index, title, and url.
    async fn list_tabs(&self) -> Result<String, String> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| format!("Failed to list pages: {e}"))?;
        let mut items = Vec::new();
        for (i, page) in pages.iter().enumerate() {
            let url = page
                .url()
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "about:blank".to_string());
            let title = page.get_title().await.ok().flatten().unwrap_or_default();
            items.push(serde_json::json!({ "index": i, "title": title, "url": url }));
        }
        Ok(serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string()))
    }

    /// Activates the tab at the given index and sets it as the active page.
    async fn activate_tab(&mut self, index: usize) -> Result<String, String> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| format!("Failed to list pages: {e}"))?;
        let page = pages
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("Tab index {index} not found"))?;
        page.activate()
            .await
            .map_err(|e| format!("Failed to activate tab: {e}"))?;
        let title = page.get_title().await.ok().flatten().unwrap_or_default();
        self.active_page = Some(page);
        Ok(title)
    }

    /// Returns the active page, creating a blank one if none exists.
    async fn ensure_page(&mut self) -> Result<(), String> {
        if self.active_page.is_none() {
            let page = self
                .browser
                .new_page("about:blank")
                .await
                .map_err(|e| format!("Failed to create page: {e}"))?;
            self.active_page = Some(page);
        }
        Ok(())
    }

    /// Opens `url` in a temporary page, runs `callback`, closes the page,
    /// and restores the previously active page.
    async fn ephemeral_page<F, T>(&self, url: &str, callback: F) -> Result<T, String>
    where
        F: for<'a> FnOnce(&'a chromiumoxide::Page) -> BoxFuture<'a, Result<T, String>>,
    {
        let prev = self.active_page.as_ref().cloned();
        let page = self
            .browser
            .new_page(url)
            .await
            .map_err(|e| format!("Failed to open page: {e}"))?;
        let _ = page
            .evaluate("Object.defineProperty(navigator,'webdriver',{get:()=>undefined})")
            .await;
        let _ = tokio::time::timeout(Duration::from_secs(5), page.wait_for_navigation()).await;
        let result = callback(&page).await;
        let _ = page.close().await;
        if let Some(prev_page) = prev {
            let _ = prev_page.activate().await;
        }
        result
    }
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
        std::path::PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
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
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .viewport(None)
        .build()
        .map_err(|e| format!("Browser config error: {e}"))?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("Failed to launch Chrome: {e}"))?;

    let handler_task = tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    Ok(BrowserState {
        browser,
        active_page: None,
        _handler: handler_task,
    })
}

// ---------------------------------------------------------------------------
// Factory — creates all browser tools sharing one browser instance
// ---------------------------------------------------------------------------

pub fn create_browser_tools() -> (SharedBrowser, Vec<Box<dyn Tool>>) {
    let shared: SharedBrowser = Arc::new(Mutex::new(None));
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(BrowserSearchTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserNavigateTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserGetUrlTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserGetTextTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserFindTextTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserGetElementTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserQuerySelectorAllTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserGetLinksTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserClickTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserTypeTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserScreenshotTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserCloseTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserOpenTabTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserCloseTabTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserListTabsTool {
            shared: Arc::clone(&shared),
        }),
        Box::new(BrowserActivateTabTool {
            shared: Arc::clone(&shared),
        }),
    ];
    (shared, tools)
}

// ---------------------------------------------------------------------------
// browser_search
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
         search box. Use this whenever you need to search for something. \
         Returns a JSON array of {title, url, description} objects."
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
        match state
            .ephemeral_page(&url, |page| {
                Box::pin(async move {
                    let js = r#"(function() {
                        var rso = document.querySelector('#rso');
                        if (!rso) return '[]';
                        var items = rso.querySelectorAll('[data-rpos]');
                        var results = [];
                        items.forEach(function(item) {
                            var h3 = item.querySelector('h3');
                            if (!h3) return;
                            var anchor = h3.closest('a');
                            if (!anchor) return;
                            var url = anchor.href || '';
                            var title = (h3.innerText || h3.textContent || '').trim();
                            var descEl = item.querySelector('div[style*="-webkit-line-clamp"]');
                            var desc = descEl
                                ? (descEl.innerText || descEl.textContent || '').trim()
                                : '';
                            if (title) results.push({ title: title, url: url, description: desc });
                        });
                        return JSON.stringify(results);
                    })()"#;
                    match page.evaluate(js).await {
                        Ok(val) => Ok(val
                            .into_value::<String>()
                            .unwrap_or_else(|_| "[]".to_string())),
                        Err(e) => Err(format!("Error extracting results: {e}")),
                    }
                })
            })
            .await
        {
            Ok(json) => json,
            Err(e) => format!("Search error: {e}"),
        }
    }
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

        let state = guard.as_mut().unwrap();
        if let Err(e) = state.ensure_page().await {
            return e;
        }
        let page = state.active_page.as_ref().unwrap();

        match page.goto(&url).await {
            Ok(_) => {
                let _ =
                    tokio::time::timeout(Duration::from_secs(5), page.wait_for_navigation()).await;
                let _ = page
                    .evaluate("Object.defineProperty(navigator,'webdriver',{get:()=>undefined})")
                    .await;
                let title = page
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
        let Some(page) = state.active_page.as_ref() else {
            return "Browser not open. Use browser_navigate to open a URL first.".to_string();
        };

        match page
            .evaluate("document.body ? document.body.innerText : ''")
            .await
        {
            Ok(result) => {
                let text = result.into_value::<String>().unwrap_or_default();
                if text.len() > max_chars {
                    let boundary = (0..=max_chars)
                        .rev()
                        .find(|&i| text.is_char_boundary(i))
                        .unwrap_or(0);
                    format!("{}…[truncated at {max_chars} bytes]", &text[..boundary])
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
        let Some(page) = state.active_page.as_ref() else {
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

        match page.evaluate(js.as_str()).await {
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

        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open.".to_string();
        };
        let Some(page) = state.active_page.as_ref() else {
            return "Browser not open.".to_string();
        };

        match page.find_element(&selector).await {
            Ok(elem) => match elem.click().await {
                Ok(_) => {
                    let navigated =
                        tokio::time::timeout(Duration::from_secs(4), page.wait_for_navigation())
                            .await
                            .is_ok();
                    if navigated {
                        format!("Clicked '{selector}' — page navigated")
                    } else {
                        format!("Clicked '{selector}' — no navigation detected")
                    }
                }
                Err(e) => format!("Click error on '{selector}': {e}"),
            },
            Err(e) => format!("Element not found '{selector}': {e}"),
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

        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open.".to_string();
        };
        let Some(page) = state.active_page.as_ref() else {
            return "Browser not open.".to_string();
        };

        let safe_selector = selector.replace('\'', "\\'");
        let clear_js = format!(
            "(function(){{ var el=document.querySelector('{safe_selector}'); \
             if(el){{el.value='';el.dispatchEvent(new Event('input',{{bubbles:true}}));return true;}} \
             return false; }})()"
        );
        let _ = page.evaluate(clear_js.as_str()).await;

        match page.find_element(&selector).await {
            Ok(elem) => match elem.type_str(&text).await {
                Ok(_) => {
                    if submit {
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
                            }})();"#
                        );
                        let _ = page.evaluate(submit_js.as_str()).await;
                        let _ = tokio::time::timeout(
                            Duration::from_secs(6),
                            page.wait_for_navigation(),
                        )
                        .await;
                        format!("Typed '{text}' into '{selector}' and submitted")
                    } else {
                        format!("Typed '{text}' into '{selector}'")
                    }
                }
                Err(e) => format!("Type error on '{selector}': {e}"),
            },
            Err(e) => format!("Element not found '{selector}': {e}"),
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
        let Some(page) = state.active_page.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };

        match page.screenshot(ScreenshotParams::builder().build()).await {
            Ok(bytes) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
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
        *guard = None;
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
        let Some(page) = state.active_page.as_ref() else {
            return "Browser not open.".to_string();
        };

        match page.url().await {
            Ok(Some(url)) => url,
            Ok(None) => "about:blank".to_string(),
            Err(e) => format!("Error getting URL: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_find_text  (grep-like search within page innerText)
// ---------------------------------------------------------------------------

pub struct BrowserFindTextTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserFindTextTool {
    fn name(&self) -> &str {
        "browser_find_text"
    }

    fn description(&self) -> &str {
        "Search for a text pattern within the current page and return matching lines with \
         surrounding context (like grep -C). Use this instead of browser_get_text when you \
         are looking for specific information — it avoids reading the entire page."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Text to search for (case-insensitive)"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of surrounding lines to include before/after each match (default: 3)"
                },
                "max_matches": {
                    "type": "integer",
                    "description": "Maximum number of matches to return (default: 10)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return "Error: 'pattern' is required".to_string(),
        };
        let context_lines = input
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;
        let max_matches = input
            .get("max_matches")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };
        let Some(page) = state.active_page.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };

        let pattern_json = serde_json::to_string(&pattern).unwrap_or_default();
        let js = format!(
            r#"(function() {{
                var pattern = {pattern_json};
                var contextLines = {context_lines};
                var maxMatches = {max_matches};
                var text = document.body ? document.body.innerText : '';
                var lines = text.split('\n');
                var results = [];
                var lastEnd = -1;
                for (var i = 0; i < lines.length && results.length < maxMatches; i++) {{
                    if (lines[i].toLowerCase().indexOf(pattern.toLowerCase()) !== -1) {{
                        var start = Math.max(0, i - contextLines);
                        var end = Math.min(lines.length - 1, i + contextLines);
                        if (start <= lastEnd) start = lastEnd + 1;
                        if (start > end) continue;
                        lastEnd = end;
                        var chunk = lines.slice(start, end + 1).map(function(l, idx) {{
                            return (start + idx === i ? '>>>' : '   ') + ' ' + l.trim();
                        }}).filter(function(l) {{ return l.trim().length > 3; }}).join('\n');
                        if (chunk.trim()) results.push(chunk);
                    }}
                }}
                if (results.length === 0) return 'No matches found for: ' + pattern;
                return results.join('\n---\n') + '\n(' + results.length + ' match(es))';
            }})()"#
        );

        match page.evaluate(js.as_str()).await {
            Ok(result) => result.into_value::<String>().unwrap_or_default(),
            Err(e) => format!("Error searching page: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_get_element  (get innerText of a specific DOM element)
// ---------------------------------------------------------------------------

pub struct BrowserGetElementTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserGetElementTool {
    fn name(&self) -> &str {
        "browser_get_element"
    }

    fn description(&self) -> &str {
        "Get the text content of a specific DOM element (CSS selector). \
         Use this to extract a focused section of the page (e.g. 'main', 'article', \
         '#search-results', '.listing') instead of reading the entire page with \
         browser_get_text. Prefer this tool when the page structure is known."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "CSS selector of the element to read (e.g. 'main', 'article', '#content', '.results')"
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Maximum characters to return (default: 4000)"
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
        let max_chars = input
            .get("max_chars")
            .and_then(|v| v.as_u64())
            .unwrap_or(4000) as usize;

        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };
        let Some(page) = state.active_page.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };

        let selector_json = serde_json::to_string(&selector).unwrap_or_default();
        let js = format!(
            r#"(function() {{
                var el = document.querySelector({selector_json});
                if (!el) return 'Element not found: {selector}';
                return el.innerText || el.textContent || '';
            }})()"#
        );

        match page.evaluate(js.as_str()).await {
            Ok(result) => {
                let text = result.into_value::<String>().unwrap_or_default();
                if text.len() > max_chars {
                    let boundary = (0..=max_chars)
                        .rev()
                        .find(|&i| text.is_char_boundary(i))
                        .unwrap_or(0);
                    format!("{}…[truncated at {max_chars} bytes]", &text[..boundary])
                } else {
                    text
                }
            }
            Err(e) => format!("Error getting element text: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_query_selector_all  (outerHTML of every element matching a CSS selector)
// ---------------------------------------------------------------------------

pub struct BrowserQuerySelectorAllTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserQuerySelectorAllTool {
    fn name(&self) -> &str {
        "browser_query_selector_all"
    }

    fn description(&self) -> &str {
        "Returns the outerHTML of every DOM element that matches a CSS selector — \
         equivalent to Array.from(document.querySelectorAll(selector)).map(e => e.outerHTML). \
         Useful for extracting repeated structures such as search result cards, table rows, \
         or list items. Returns a JSON array of HTML strings."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "CSS selector (e.g. 'li.result', 'tr', '.card', 'a[data-id]')"
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Maximum total characters to return across all elements (default: 8000)"
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
        let max_chars = input
            .get("max_chars")
            .and_then(|v| v.as_u64())
            .unwrap_or(8000) as usize;

        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };
        let Some(page) = state.active_page.as_ref() else {
            return "Browser not open. Use browser_navigate first.".to_string();
        };

        let selector_json = serde_json::to_string(&selector).unwrap_or_default();
        let js = format!(
            r#"JSON.stringify(
                Array.from(document.querySelectorAll({selector_json}))
                    .map(function(e) {{ return e.outerHTML; }})
            )"#
        );

        match page.evaluate(js.as_str()).await {
            Ok(result) => {
                let json_str = result.into_value::<String>().unwrap_or_default();
                if json_str.len() > max_chars {
                    let boundary = (0..=max_chars)
                        .rev()
                        .find(|&i| json_str.is_char_boundary(i))
                        .unwrap_or(0);
                    format!("{}…[truncated at {max_chars} chars]", &json_str[..boundary])
                } else if json_str == "[]" {
                    format!("No elements matched selector: {selector}")
                } else {
                    json_str
                }
            }
            Err(e) => format!("Error running querySelectorAll: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_open_tab
// ---------------------------------------------------------------------------

pub struct BrowserOpenTabTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserOpenTabTool {
    fn name(&self) -> &str {
        "browser_open_tab"
    }

    fn description(&self) -> &str {
        "Opens a URL in a NEW browser tab and makes it the active tab. \
         Use this instead of browser_navigate when you want to preserve the current page. \
         Each agent session should open its own tab with this tool and close it with \
         browser_close_tab when finished."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Full URL to open in a new tab (e.g. https://www.example.com)"
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

        let state = guard.as_mut().unwrap();
        match state.open(&url).await {
            Ok(title) => format!("Opened new tab: {url}\nPage title: {title}"),
            Err(e) => format!("Error opening tab: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_close_tab
// ---------------------------------------------------------------------------

pub struct BrowserCloseTabTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserCloseTabTool {
    fn name(&self) -> &str {
        "browser_close_tab"
    }

    fn description(&self) -> &str {
        "Closes the current active tab and switches focus to another open tab. \
         If no other tabs exist, a blank tab is created. \
         Use this to clean up after a session that opened its own tab with browser_open_tab."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: serde_json::Value) -> String {
        let mut guard = self.shared.lock().await;
        let Some(state) = guard.as_mut() else {
            return "Browser not open.".to_string();
        };
        match state.close_page().await {
            Ok(()) => "Tab closed.".to_string(),
            Err(e) => format!("Error closing tab: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_list_tabs
// ---------------------------------------------------------------------------

pub struct BrowserListTabsTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserListTabsTool {
    fn name(&self) -> &str {
        "browser_list_tabs"
    }

    fn description(&self) -> &str {
        "Returns a JSON array of all open browser tabs with their index, title, and url. \
         Use the index value with browser_activate_tab to switch to a specific tab."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _input: serde_json::Value) -> String {
        let guard = self.shared.lock().await;
        let Some(state) = guard.as_ref() else {
            return "Browser not open.".to_string();
        };
        match state.list_tabs().await {
            Ok(json) => json,
            Err(e) => format!("Error listing tabs: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// browser_activate_tab
// ---------------------------------------------------------------------------

pub struct BrowserActivateTabTool {
    pub shared: SharedBrowser,
}

#[async_trait]
impl Tool for BrowserActivateTabTool {
    fn name(&self) -> &str {
        "browser_activate_tab"
    }

    fn description(&self) -> &str {
        "Switches the active tab to the tab at the given index. \
         Use browser_list_tabs first to find the index of the tab you want."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "index": {
                    "type": "integer",
                    "description": "Zero-based index of the tab to activate (from browser_list_tabs)"
                }
            },
            "required": ["index"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> String {
        let index = match input.get("index").and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => return "Error: 'index' is required".to_string(),
        };

        let mut guard = self.shared.lock().await;
        let Some(state) = guard.as_mut() else {
            return "Browser not open.".to_string();
        };
        match state.activate_tab(index).await {
            Ok(title) => format!("Activated tab {index}: {title}"),
            Err(e) => format!("Error activating tab: {e}"),
        }
    }
}
