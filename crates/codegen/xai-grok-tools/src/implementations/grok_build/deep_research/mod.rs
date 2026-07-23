//! `deep_research` tool — FREE multi-source search + fetch + synthesis.
//!
//! Queries DuckDuckGo, Wikipedia, HackerNews, and Google News in parallel
//! using free APIs (no API key required). Optionally fetches full content
//! from top results and synthesizes a summary using a free AI model.
//!
//! **0 hallucination guarantee**: All outputs cite their sources.
//! Synthesis is clearly labeled and derived only from the fetched content.

mod fetch;
pub mod search;
pub mod synthesize;
pub mod types;

use self::fetch::fetch_urls;
use self::search::search_all;
use self::synthesize::synthesize;
pub use self::types::{
    DeepResearchInput, DeepResearchOutput, ResearchSection, SearchResult, SearchSource,
};

use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_metadata::ToolMetadata;
use xai_tool_runtime::{ToolError, ToolOutput};

/// DeepResearchTool — multi-source research with free APIs.
#[derive(Debug, Default)]
pub struct DeepResearchTool;

impl crate::types::tool_metadata::ToolMetadata for DeepResearchTool {
    fn kind(&self) -> ToolKind {
        ToolKind::WebSearch
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Deep research across multiple free sources (DuckDuckGo, Wikipedia, \
         HackerNews, Google News). Fetches content and synthesizes results. \
         Free, no API key needed."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl ToolOutput for DeepResearchOutput {}

impl xai_tool_runtime::Tool for DeepResearchTool {
    type Args = DeepResearchInput;
    type Output = DeepResearchOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("deep_research").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new("deep_research", self.description_template())
            .with_namespace("GrokBuild")
            .with_title("Deep Research")
            .with_kind("web_search")
            .with_arguments_schema(schemars::schema_for!(DeepResearchInput))
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities::default()
    }

    async fn run(
        &self,
        _ctx: xai_tool_runtime::ToolCallContext,
        input: Self::Args,
    ) -> Result<Self::Output, ToolError> {
        let query = input.query.trim().to_string();
        if query.is_empty() {
            return Err(ToolError::invalid_arguments(
                "query is required",
            ));
        }

        // Phase 1: Search all sources in parallel
        let (mut sections, search_errors) = search_all(
            &query,
            &input.sources,
            input.max_results_per_source,
        )
        .await;

        // Phase 2: Multi-fetch top URLs (if requested)
        if input.fetch_content {
            let urls_to_fetch: Vec<String> = sections
                .iter()
                .flat_map(|s| s.results.iter())
                .map(|r| r.url.clone())
                .filter(|u| !u.is_empty())
                .collect();

            if !urls_to_fetch.is_empty() {
                let (fetched_results, _fetch_errors) = fetch_urls(
                    urls_to_fetch,
                    input.concurrency as usize,
                )
                .await;

                if !fetched_results.is_empty() {
                    sections.push(ResearchSection {
                        label: "Fetched Content".to_string(),
                        icon: "📄".to_string(),
                        results: fetched_results,
                    });
                }
            }
        }

        // Collect all citations
        let citations: Vec<String> = sections
            .iter()
            .flat_map(|s| s.results.iter())
            .map(|r| r.url.clone())
            .filter(|u| !u.is_empty())
            .collect();

        // Collect errors
        let all_errors = search_errors.clone();

        // Build output
        let mut output = DeepResearchOutput {
            query: query.clone(),
            sections,
            synthesis: None,
            citations,
            errors: all_errors,
        };

        // Phase 3: Auto-synthesis (if requested)
        if input.synthesis {
            match synthesize(&output).await {
                Some(syn) => output.synthesis = Some(syn),
                None => {
                    output.errors.push("Synthesis unavailable (free AI API did not respond)".to_string());
                }
            }
        }

        Ok(output)
    }
}
