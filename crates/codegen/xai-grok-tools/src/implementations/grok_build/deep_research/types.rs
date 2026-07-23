//! Types for the DeepResearchTool — multi-source search + fetch + synthesis.

use serde::{Deserialize, Serialize};

/// Which search sources to query.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchSource {
    /// DuckDuckGo HTML search (general web results)
    DuckDuckGo,
    /// Wikipedia API (encyclopedic content)
    Wikipedia,
    /// HackerNews Algolia API (tech news, discussions)
    HackerNews,
    /// Google News RSS (news articles)
    GoogleNews,
    /// All available sources
    All,
}

impl SearchSource {
    pub fn variants() -> Vec<&'static str> {
        vec![
            "duck_duck_go",
            "wikipedia",
            "hacker_news",
            "google_news",
            "all",
        ]
    }
}

/// Input for the DeepResearchTool.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeepResearchInput {
    /// The research query or question.
    pub query: String,

    /// Sources to search. Defaults to All.
    #[serde(default)]
    pub sources: Vec<SearchSource>,

    /// Max results per source (default: 5).
    #[serde(default = "default_max_results")]
    pub max_results_per_source: u8,

    /// Whether to deep-fetch the top result URLs for full content.
    #[serde(default = "default_fetch_content")]
    pub fetch_content: bool,

    /// Maximum URLs to fetch concurrently (default: 5, max: 20).
    #[serde(default = "default_concurrency")]
    pub concurrency: u8,

    /// Whether to synthesize results using a free AI model.
    #[serde(default = "default_synthesis")]
    pub synthesis: bool,
}

fn default_max_results() -> u8 { 5 }
fn default_fetch_content() -> bool { true }
fn default_concurrency() -> u8 { 5 }
fn default_synthesis() -> bool { true }

/// A single search result from any source.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchResult {
    /// Source identifier (duckduckgo, wikipedia, hackernews, google_news).
    pub source: String,
    /// Result title.
    pub title: String,
    /// Result URL.
    pub url: String,
    /// Content snippet or excerpt.
    pub snippet: String,
    /// Publication date if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
    /// Author/source name if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
}

/// Section of the research output grouped by source.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSection {
    /// Source name (display label).
    pub label: String,
    /// Emoji icon for display.
    pub icon: String,
    /// Results from this source.
    pub results: Vec<SearchResult>,
}

/// Final output of DeepResearchTool.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeepResearchOutput {
    /// Original query.
    pub query: String,
    /// Research sections grouped by source.
    pub sections: Vec<ResearchSection>,
    /// Optional AI-generated synthesis summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthesis: Option<String>,
    /// All unique citations from all sources.
    pub citations: Vec<String>,
    /// Error messages from any failed sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl DeepResearchOutput {
    /// Convert to markdown-ready prompt text.
    pub fn to_prompt_format(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# 🔍 Deep Research: {}\n\n", self.query));

        for section in &self.sections {
            if section.results.is_empty() {
                continue;
            }
            out.push_str(&format!("## {} {}\n\n", section.icon, section.label));
            for (i, result) in section.results.iter().enumerate() {
                out.push_str(&format!(
                    "{}. [{}]({})\n",
                    i + 1,
                    result.title,
                    result.url
                ));
                out.push_str(&format!("   {}\n", result.snippet));
                if let Some(ref date) = result.published {
                    out.push_str(&format!("   📅 {}\n", date));
                }
                if let Some(ref author) = result.attribution {
                    out.push_str(&format!("   👤 {}\n", author));
                }
                out.push('\n');
            }
        }

        if let Some(ref syn) = self.synthesis {
            out.push_str("---\n\n");
            out.push_str("## 📝 Synthesis\n\n");
            out.push_str(syn);
            out.push('\n');
        }

        out.push_str("---\n\n");
        out.push_str("## 📎 Citations\n\n");
        for (i, url) in self.citations.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, url));
        }

        out
    }
}
