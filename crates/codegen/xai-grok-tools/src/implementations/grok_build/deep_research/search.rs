//! Multi-source search engine.
//!
//! Queries 4 free APIs in parallel:
//! - DuckDuckGo HTML (scrape)
//! - Wikipedia API (JSON)
//! - HackerNews Algolia (JSON)
//! - Google News RSS

use super::types::{ResearchSection, SearchResult};
use std::time::Duration;

/// Search all configured sources in parallel.
pub async fn search_all(
    query: &str,
    sources: &[super::types::SearchSource],
    max_results: u8,
) -> (Vec<ResearchSection>, Vec<String>) {
    let client = reqwest::Client::builder()
        .user_agent("grok-free-research/1.0")
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let use_all = sources.is_empty() || sources.iter().any(|s| matches!(s, super::types::SearchSource::All));
    let mut sections = Vec::new();
    let mut errors = Vec::new();
    let mut handles = Vec::new();
    let max = max_results.max(1).min(20) as usize;

    // DuckDuckGo
    if use_all || sources.iter().any(|s| matches!(s, super::types::SearchSource::DuckDuckGo)) {
        let c = client.clone();
        let q = query.to_string();
        handles.push(tokio::spawn(async move {
            search_duckduckgo(&c, &q, max).await
        }));
    }

    // Wikipedia
    if use_all || sources.iter().any(|s| matches!(s, super::types::SearchSource::Wikipedia)) {
        let c = client.clone();
        let q = query.to_string();
        handles.push(tokio::spawn(async move {
            search_wikipedia(&c, &q, max).await
        }));
    }

    // HackerNews
    if use_all || sources.iter().any(|s| matches!(s, super::types::SearchSource::HackerNews)) {
        let c = client.clone();
        let q = query.to_string();
        handles.push(tokio::spawn(async move {
            search_hackernews(&c, &q, max).await
        }));
    }

    // Google News
    if use_all || sources.iter().any(|s| matches!(s, super::types::SearchSource::GoogleNews)) {
        let c = client.clone();
        let q = query.to_string();
        handles.push(tokio::spawn(async move {
            search_google_news(&c, &q, max).await
        }));
    }

    for handle in handles {
        match handle.await {
            Ok(Ok((section, errs))) => {
                sections.push(section);
                errors.extend(errs);
            }
            Ok(Err(e)) => {
                errors.push(format!("Source failed: {e}"));
            }
            Err(e) => {
                errors.push(format!("Task panicked: {e}"));
            }
        }
    }

    (sections, errors)
}

/// Search DuckDuckGo via HTML endpoint.
async fn search_duckduckgo(
    client: &reqwest::Client,
    query: &str,
    max: usize,
) -> Result<(ResearchSection, Vec<String>), String> {
    let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding(query));
    let resp = client.get(&url).send().await.map_err(|e| format!("DDG request failed: {e}"))?;
    let html = resp.text().await.map_err(|e| format!("DDG body failed: {e}"))?;

    let document = scraper::Html::parse_document(&html);

    // Selector for result links: .results .result__a
    let result_sel = scraper::Selector::parse(".result").map_err(|_| "Invalid selector")?;
    let link_sel = scraper::Selector::parse(".result__a").map_err(|_| "Invalid selector")?;
    let snippet_sel = scraper::Selector::parse(".result__snippet").map_err(|_| "Invalid selector")?;

    let mut results = Vec::new();

    for result_elem in document.select(&result_sel).take(max) {
        let link_elem = result_elem.select(&link_sel).next();
        let snippet_elem = result_elem.select(&snippet_sel).next();

        let title = link_elem
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Extract URL from the href attribute (DDG redirect: //duckduckgo.com/l/?uddg=...)
        let url = link_elem
            .and_then(|e| e.value().attr("href"))
            .map(|h| decode_ddg_url(h))
            .unwrap_or_default();

        let snippet = snippet_elem
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if !title.is_empty() {
            results.push(SearchResult {
                source: "duckduckgo".to_string(),
                title,
                url,
                snippet,
                published: None,
                attribution: None,
            });
        }
    }

    Ok((
        ResearchSection {
            label: "DuckDuckGo".to_string(),
            icon: "🌐".to_string(),
            results,
        },
        vec![],
    ))
}

/// Search Wikipedia via REST API.
async fn search_wikipedia(
    client: &reqwest::Client,
    query: &str,
    max: usize,
) -> Result<(ResearchSection, Vec<String>), String> {
    let url = format!(
        "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit={}",
        urlencoding(query),
        max.min(20)
    );
    let resp = client.get(&url).send().await.map_err(|e| format!("Wikipedia request failed: {e}"))?;
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("Wikipedia JSON failed: {e}"))?;

    let mut results = Vec::new();
    let mut errors = Vec::new();

    if let Some(search) = json["query"]["search"].as_array() {
        for item in search.iter().take(max) {
            let title = item["title"].as_str().unwrap_or("").to_string();
            let snippet_html = item["snippet"].as_str().unwrap_or("");
            // Strip HTML tags from snippet
            let snippet = strip_html_tags(snippet_html);
            let url = format!("https://en.wikipedia.org/wiki/{}", urlencoding(&title));
            let timestamp = item["timestamp"].as_str().map(|s| {
                let d = &s[..10]; // "2024-01-15T..."
                d.to_string()
            });

            if !title.is_empty() {
                results.push(SearchResult {
                    source: "wikipedia".to_string(),
                    title,
                    url,
                    snippet,
                    published: timestamp,
                    attribution: Some("Wikipedia".to_string()),
                });
            }
        }
    } else {
        errors.push("Wikipedia: no results found".to_string());
    }

    Ok((
        ResearchSection {
            label: "Wikipedia".to_string(),
            icon: "📚".to_string(),
            results,
        },
        errors,
    ))
}

/// Search HackerNews via Algolia API.
async fn search_hackernews(
    client: &reqwest::Client,
    query: &str,
    max: usize,
) -> Result<(ResearchSection, Vec<String>), String> {
    let url = format!(
        "https://hn.algolia.com/api/v1/search?query={}&hitsPerPage={}",
        urlencoding(query),
        max.min(50)
    );
    let resp = client.get(&url).send().await.map_err(|e| format!("HN request failed: {e}"))?;
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("HN JSON failed: {e}"))?;

    let mut results = Vec::new();

    if let Some(hits) = json["hits"].as_array() {
        for hit in hits.iter().take(max) {
            let title = hit["title"].as_str().unwrap_or("").to_string();
            let url = hit["url"].as_str()
                .or_else(|| hit["story_url"].as_str())
                .unwrap_or("")
                .to_string();
            let hn_url = format!("https://news.ycombinator.com/item?id={}", hit["objectID"].as_str().unwrap_or(""));
            let points = hit["points"].as_i64().unwrap_or(0);
            let num_comments = hit["num_comments"].as_i64().unwrap_or(0);
            let author = hit["author"].as_str().unwrap_or("").to_string();
            let created_at = hit["created_at"].as_str().unwrap_or("").to_string();
            let snippet = format!("{} points | {} comments | by {}", points, num_comments, author);

            if !title.is_empty() {
                results.push(SearchResult {
                    source: "hackernews".to_string(),
                    title,
                    url: if url.is_empty() { hn_url } else { url },
                    snippet,
                    published: Some(created_at),
                    attribution: Some(format!("HN by {}", author)),
                });
            }
        }
    }

    Ok((
        ResearchSection {
            label: "HackerNews".to_string(),
            icon: "📰".to_string(),
            results,
        },
        vec![],
    ))
}

/// Search Google News via RSS.
async fn search_google_news(
    client: &reqwest::Client,
    query: &str,
    max: usize,
) -> Result<(ResearchSection, Vec<String>), String> {
    let url = format!(
        "https://news.google.com/rss/search?q={}&hl=en-US&gl=US&ceid=US:en",
        urlencoding(query)
    );
    let resp = client.get(&url).send().await.map_err(|e| format!("Google News request failed: {e}"))?;
    let xml = resp.text().await.map_err(|e| format!("Google News body failed: {e}"))?;

    // Parse RSS XML using quick-xml (already in workspace)
    let mut results = Vec::new();

    // Simple RSS parser with quick-xml reader
    let mut reader = quick_xml::Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut in_item = false;
    let mut in_title = false;
    let mut in_link = false;
    let mut in_pub_date = false;
    let mut in_source = false;
    let mut current_title = String::new();
    let mut current_link = String::new();
    let mut current_date = String::new();
    let mut current_source = String::new();

    use quick_xml::events::Event;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "item" => in_item = true,
                    "title" if in_item => in_title = true,
                    "link" if in_item => in_link = true,
                    "pubDate" if in_item => in_pub_date = true,
                    "source" if in_item => in_source = true,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                if in_title { current_title.push_str(&text); }
                if in_link { current_link.push_str(&text); }
                if in_pub_date { current_date.push_str(&text); }
                if in_source { current_source.push_str(&text); }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "item" => {
                        if !current_title.is_empty() {
                            results.push(SearchResult {
                                source: "google_news".to_string(),
                                title: std::mem::take(&mut current_title),
                                url: std::mem::take(&mut current_link),
                                snippet: String::new(),
                                published: Some(std::mem::take(&mut current_date)),
                                attribution: if current_source.is_empty() { None } else { Some(std::mem::take(&mut current_source)) },
                            });
                        }
                        in_item = false;
                    }
                    "title" => in_title = false,
                    "link" => in_link = false,
                    "pubDate" => in_pub_date = false,
                    "source" => in_source = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(format!("XML parse error: {e}"));
            }
            _ => {}
        }

        if results.len() >= max {
            break;
        }
    }

    Ok((
        ResearchSection {
            label: "Google News".to_string(),
            icon: "📰".to_string(),
            results,
        },
        vec![],
    ))
}

/// Simple URL-encoding (replaces spaces etc.)
fn urlencoding(s: &str) -> String {
    urlencoding_inner(s)
}

fn urlencoding_inner(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

/// Decode DuckDuckGo redirect URL to real URL.
fn decode_ddg_url(href: &str) -> String {
    // DDG redirect format: //duckduckgo.com/l/?uddg=<base64url>
    if let Some(encoded) = href.split("uddg=").nth(1) {
        let cleaned = encoded.split('&').next().unwrap_or(encoded);
        // Base64 URL-safe decode
        use base64::Engine as _;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        if let Ok(decoded) = engine.decode(cleaned) {
            if let Ok(decoded_str) = String::from_utf8(decoded) {
                return decoded_str;
            }
        }
    }
    href.to_string()
}

/// Strip HTML tags from a string.
fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // Unescape common HTML entities
    out = out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    out
}
