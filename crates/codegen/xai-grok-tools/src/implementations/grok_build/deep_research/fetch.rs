//! Multi-fetch engine — fetches multiple URLs concurrently.
//!
//! Uses `FuturesUnordered` with a semaphore for concurrency control.
//! Unlimited URLs (limited by RAM/resources, not hardcoded).

use super::types::SearchResult;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Fetch multiple URLs and return their markdown content.
pub async fn fetch_urls(
    urls: Vec<String>,
    max_concurrency: usize,
) -> (Vec<SearchResult>, Vec<String>) {
    if urls.is_empty() {
        return (vec![], vec![]);
    }

    let concurrency = max_concurrency.max(1).min(50); // Sanity cap at 50
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let client = Arc::new(
        reqwest::Client::builder()
            .user_agent("grok-free-research/1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default(),
    );

    let mut tasks = FuturesUnordered::new();

    for url in urls {
        let client = client.clone();
        let sem = semaphore.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            fetch_single(client, url).await
        }));
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();

    while let Some(result) = tasks.next().await {
        match result {
            Ok(Ok(search_result)) => {
                if !search_result.title.is_empty() || !search_result.snippet.is_empty() {
                    results.push(search_result);
                }
            }
            Ok(Err(e)) => {
                errors.push(e);
            }
            Err(e) => {
                errors.push(format!("Fetch task panicked: {e}"));
            }
        }
    }

    (results, errors)
}

/// Fetch a single URL and extract content as markdown.
async fn fetch_single(
    client: Arc<reqwest::Client>,
    url: String,
) -> Result<SearchResult, String> {
    let resp = client
        .get(&url)
        .header("Accept", "text/html, text/markdown, text/plain, */*")
        .send()
        .await
        .map_err(|e| format!("Fetch {url}: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("Fetch {url}: HTTP {status}",));
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Fetch {url} body: {e}"))?;

    let max_bytes = 100_000; // 100KB max per URL
    let truncated = body.len() > max_bytes;
    let content = if truncated {
        format!("{}...\n\n[Content truncated at {max_bytes} bytes]", &body[..max_bytes])
    } else {
        body
    };

    // Simple HTML → plain text conversion if HTML
    let snippet: String = if content_type.contains("html") {
        let document = scraper::Html::parse_document(&content);
        let body_sel = scraper::Selector::parse("body, article, main").ok();
        if let Some(sel) = body_sel {
            if let Some(body_elem) = document.select(&sel).next() {
                let text: String = body_elem.text().collect::<Vec<_>>().join(" ");
                // Clean whitespace
                let cleaned: Vec<String> = text
                    .split_whitespace()
                    .map(|w| w.to_string())
                    .collect();
                cleaned.join(" ").chars().take(2000).collect()
            } else {
                // Fallback: grab all text
                let text: String = document.root_element().text().collect();
                let cleaned: Vec<String> = text.split_whitespace().map(|w| w.to_string()).collect();
                cleaned.join(" ").chars().take(2000).collect()
            }
        } else {
            content.chars().take(2000).collect()
        }
    } else {
        content.chars().take(2000).collect()
    };

    Ok(SearchResult {
        source: "web_fetch".to_string(),
        title: url.clone(),
        url,
        snippet: format!("[{} chars, HTTP {status}] {}", snippet.len(), snippet),
        published: None,
        attribution: None,
    })
}
