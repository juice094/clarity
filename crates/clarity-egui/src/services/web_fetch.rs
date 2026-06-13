use regex::Regex;
use std::sync::LazyLock;

/// Compile a literal regex pattern that is valid by construction.
///
// SAFE: only called with hard-coded static patterns below.
#[allow(clippy::unwrap_used)]
fn compile_regex(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| compile_regex(r"<[^>]+>"));

static BLOCK_TAG_RES: LazyLock<Vec<(Regex, Regex)>> = LazyLock::new(|| {
    ["p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "li", "tr"]
        .iter()
        .map(|tag| {
            let open = compile_regex(&format!(r"<{}[^>]*>", regex::escape(tag)));
            let close = compile_regex(&format!(r"</{}>", regex::escape(tag)));
            (open, close)
        })
        .collect()
});

static TITLE_RE: LazyLock<Regex> = LazyLock::new(|| compile_regex(r"<title[^>]*>(.*?)</title>"));

/// Fetch a web page, extract its title, and convert HTML to plain text.
pub async fn fetch_web_page(url: &str) -> Result<(String, String), String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let html = response
        .text()
        .await
        .map_err(|e| format!("Failed to read body: {}", e))?;

    let title = extract_title(&html);
    let text = html_to_text(&html);

    Ok((title, text))
}

/// Simplified HTML-to-text conversion.
/// Replaces block tags with newlines, strips remaining tags, decodes entities,
/// normalizes whitespace, and caps output at ~50 KB.
fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    for (open_re, close_re) in BLOCK_TAG_RES.iter() {
        text = open_re.replace_all(&text, "\n").to_string();
        text = close_re.replace_all(&text, "\n").to_string();
    }

    text = text
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");

    text = TAG_RE.replace_all(&text, "").to_string();
    text = html_escape::decode_html_entities(&text).to_string();

    let lines: Vec<_> = text
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();
    text = lines.join("\n\n");

    if text.len() > 50000 {
        let mut end = 50000;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n\n[Content truncated due to length]");
    }

    text.trim().to_string()
}

/// Extract the `<title>` tag contents from HTML.
fn extract_title(html: &str) -> String {
    let re = TITLE_RE.captures(html);
    match re {
        Some(caps) => html_escape::decode_html_entities(caps.get(1).map_or("", |m| m.as_str()))
            .trim()
            .to_string(),
        None => "Untitled".to_string(),
    }
}
