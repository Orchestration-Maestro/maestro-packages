//! Browser automation: fetch a page through the shared HTTP client and
//! extract its security-relevant structure (title, links, forms, inputs) with
//! a small dependency-free HTML scanner, then model-summarize the page.
//!
//! This is the headless, no-JavaScript route: it reads what the server sends,
//! which is what matters for surface analysis. JavaScript-rendered content is
//! listed as an unknown, not guessed.

use serde::{Deserialize, Serialize};

use crate::ctx::CapabilityCtx;
use crate::pipeline::Config;

/// Schema tag for the browse result.
pub const SCHEMA: &str = "maestro-security/browser/1";

/// Parameters for a browse.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Visit {
    /// The URL to fetch.
    pub url: String,
    /// Model override.
    pub model: Option<String>,
}

/// One extracted link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageLink {
    /// The href.
    pub href: String,
    /// The link text, if any.
    pub text: String,
}

/// The extracted page structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    /// The requested URL.
    pub url: String,
    /// HTTP status.
    pub status: u16,
    /// The document title.
    pub title: String,
    /// Extracted links.
    pub links: Vec<PageLink>,
    /// Form action URLs and method, one entry per form.
    pub forms: Vec<String>,
    /// Input names, one per input.
    pub inputs: Vec<String>,
    /// Visible text (tags stripped), capped.
    pub text: String,
}

/// The result of a browse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capture {
    /// Schema tag.
    pub schema: &'static str,
    /// The extracted page structure.
    pub page: PageInfo,
    /// The model's security summary of the page.
    pub summary: String,
    /// Looked for but not proven (e.g. JS-rendered content).
    pub unknown: Vec<String>,
}

/// Fetch and analyze a page.
///
/// # Errors
/// Fails if the URL is empty, the fetch fails, or the model call fails.
pub fn browse(params: &Visit) -> Result<Capture, String> {
    if params.url.trim().is_empty() {
        return Err("url is required".into());
    }
    let ctx = CapabilityCtx::build(&Config::from_env(), params.model.as_deref())?;
    let (status, body) = fetch(&params.url)?;
    let page = PageInfo {
        url: params.url.clone(),
        status,
        title: extract_title(&body),
        links: extract_links(&body),
        forms: extract_forms(&body),
        inputs: extract_inputs(&body),
        text: extract_text(&body),
    };
    let (summary, unknown) = summarize(&ctx, &page)?;
    Ok(Capture {
        schema: SCHEMA,
        page,
        summary,
        unknown,
    })
}

/// Fetch a URL, returning the status and body text.
fn fetch(url: &str) -> Result<(u16, String), String> {
    let resp = maestro_http::request("GET", url, &[("user-agent", "maestro-security/1")], &[])
        .map_err(|e| format!("fetch {url}: {e}"))?;
    Ok((resp.status, String::from_utf8_lossy(&resp.body).to_string()))
}

/// Have the model summarize the page's security relevance.
fn summarize(ctx: &CapabilityCtx, page: &PageInfo) -> Result<(String, Vec<String>), String> {
    let user = format!(
        "Page: {} (HTTP {})\nTitle: {}\nLinks: {}\nForms: {}\nInputs: {}\n\n\
Summarize the security-relevant content of this page in three sentences. \
Return JSON: {{\"summary\":\"...\",\"unknown\":[\"...\"]}}.",
        page.url,
        page.status,
        page.title,
        page.links.len(),
        page.forms.len(),
        page.inputs.len()
    );
    let raw = ctx.chat(
        "You are the web surface analyst. You describe only what the fetched \
page shows.",
        &user,
        2048,
    )?;
    let json = crate::pipeline::extract_json(&raw);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return Ok((raw, vec!["javascript-rendered content not captured".into()]));
    };
    let summary = value
        .get("summary")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let unknown = value
        .pointer("/unknown")
        .and_then(serde_json::Value::as_array)
        .map_or_else(
            || vec!["javascript-rendered content not captured".into()],
            |arr| arr.iter().filter_map(|u| u.as_str().map(str::to_string)).collect(),
        );
    Ok((summary, unknown))
}

/// The document title, or empty.
fn extract_title(html: &str) -> String {
    let lower = html.to_lowercase();
    let Some(start) = lower.find("<title>") else {
        return String::new();
    };
    let Some(end) = lower[start..].find("</title>") else {
        return String::new();
    };
    html[start + "<title>".len()..start + end].trim().to_string()
}

/// Extract `<a href="...">text</a>` links.
fn extract_links(html: &str) -> Vec<PageLink> {
    let mut links = Vec::new();
    let mut rest = html;
    while let Some(pos) = find_tag(rest, "<a ") {
        let tail = &rest[pos..];
        let href = attr(tail, "href");
        let text = tag_text(tail);
        links.push(PageLink { href, text });
        rest = &rest[pos + 3..];
        if links.len() >= 200 {
            break;
        }
    }
    links
}

/// Extract `<form ...>` action+method strings.
fn extract_forms(html: &str) -> Vec<String> {
    let mut forms = Vec::new();
    let mut rest = html;
    while let Some(pos) = find_tag(rest, "<form") {
        let tail = &rest[pos..];
        let action = attr(tail, "action");
        let method = attr(tail, "method");
        forms.push(format!("{method} {action}"));
        rest = &rest[pos + 5..];
        if forms.len() >= 100 {
            break;
        }
    }
    forms
}

/// Extract `<input ... name="...">` names.
fn extract_inputs(html: &str) -> Vec<String> {
    let mut inputs = Vec::new();
    let mut rest = html;
    while let Some(pos) = find_tag(rest, "<input") {
        let tail = &rest[pos..];
        let name = attr(tail, "name");
        if !name.is_empty() {
            inputs.push(name);
        }
        rest = &rest[pos + 6..];
        if inputs.len() >= 200 {
            break;
        }
    }
    inputs
}

/// Strip tags and collapse whitespace into visible text, capped.
fn extract_text(html: &str) -> String {
    let no_tags: String = html
        .chars()
        .filter(|c| !(*c == '<' || *c == '>'))
        .collect();
    let words: Vec<&str> = no_tags.split_whitespace().collect();
    let joined = words.join(" ");
    joined.chars().take(2000).collect()
}

/// Find the next occurrence of a tag opener, case-insensitive.
fn find_tag(html: &str, tag: &str) -> Option<usize> {
    let lower = html.to_lowercase();
    lower.find(tag)
}

/// Read a quoted attribute value from a tag tail.
fn attr(tail: &str, name: &str) -> String {
    let lower = tail.to_lowercase();
    let key = format!("{name}=");
    let Some(pos) = lower.find(&key) else {
        return String::new();
    };
    let after = &tail[pos + key.len()..];
    let quote = if after.starts_with('"') {
        '"'
    } else if after.starts_with('\'') {
        '\''
    } else {
        return String::new();
    };
    let body = &after[1..];
    let end = body.find(quote).unwrap_or(body.len());
    body[..end].to_string()
}

/// The text between the tag opener and the next `>`.
fn tag_text(tail: &str) -> String {
    let Some(open) = tail.find('>') else {
        return String::new();
    };
    let inner = &tail[open + 1..];
    let Some(close) = inner.to_lowercase().find("</a>") else {
        return String::new();
    };
    inner[..close].trim().to_string()
}

#[cfg(test)]
mod tests {
    //! Unit tests for the HTML extraction helpers.

    use super::{extract_inputs, extract_links, extract_title};

    /// The title is extracted between the tags.
    #[test]
    fn extracts_title() {
        assert_eq!(extract_title("<html><head><title>Hi</title></head>"), "Hi");
    }

    /// Links are extracted with their href.
    #[test]
    fn extracts_links() {
        let html = r#"<a href="/login">Login</a><a href="/x">X</a>"#;
        let links = extract_links(html);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].href, "/login");
    }

    /// Input names are extracted.
    #[test]
    fn extracts_inputs() {
        let html = r#"<input name="user"><input name="pass">"#;
        assert_eq!(extract_inputs(html), vec!["user", "pass"]);
    }
}
