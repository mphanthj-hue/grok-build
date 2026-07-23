//! Auto-synthesis using a free AI model (opencode.ai public API).
//!
//! Sends the collected research data to a free chat completion API
//! and returns a concise synthesis. Zero cost, no API key needed.

use super::types::DeepResearchOutput;
use std::time::Duration;

/// Synthesize research results into a concise summary using a free AI model.
///
/// Uses the opencode.ai public API (same as deepseek-v4-flash-free in config).
/// If the API call fails, returns None — the research results are still useful
/// without synthesis.
pub async fn synthesize(output: &DeepResearchOutput) -> Option<String> {
    if output.sections.is_empty() {
        return None;
    }

    let prompt = build_synthesis_prompt(output);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .ok()?;

    let payload = serde_json::json!({
        "model": "deepseek-v4-flash-free",
        "messages": [
            {
                "role": "system",
                "content": "You are a research synthesis assistant. Synthesize the provided search results into a concise, factual summary. Only use information from the provided sources. Do not add external knowledge. If sources conflict, note the conflict. Output in markdown with section headings and bullet points where appropriate."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "max_tokens": 2048,
        "temperature": 0.3,
        "stream": false
    });

    let resp = match client
        .post("https://opencode.ai/zen/v1/chat/completions")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Synthesis API call failed: {e}");
            return None;
        }
    };

    let status = resp.status();
    if !status.is_success() {
        tracing::warn!("Synthesis API returned HTTP {status}");
        return None;
    }

    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("Synthesis JSON parse failed: {e}");
            return None;
        }
    };

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())?;

    Some(content)
}

/// Build a prompt for synthesis from the research output.
fn build_synthesis_prompt(output: &DeepResearchOutput) -> String {
    let mut prompt = String::from("Please synthesize the following research results:\n\n");
    prompt.push_str(&format!("**Query:** {}\n\n", output.query));

    for section in &output.sections {
        if section.results.is_empty() {
            continue;
        }
        prompt.push_str(&format!("### {} {}\n", section.icon, section.label));
        for result in &section.results {
            prompt.push_str(&format!("- [{}]({})\n", result.title, result.url));
            if !result.snippet.is_empty() {
                prompt.push_str(&format!("  {}\n", result.snippet));
            }
        }
        prompt.push('\n');
    }

    prompt.push_str("\nProvide a concise synthesis covering:\n");
    prompt.push_str("1. Key findings and themes\n");
    prompt.push_str("2. Relevant statistics or data points from the sources\n");
    prompt.push_str("3. Different perspectives if sources conflict\n");
    prompt.push_str("\nFormat as markdown with section headings and bullet points. ");
    prompt.push_str("Do NOT add information not present in the provided sources.");

    prompt
}
