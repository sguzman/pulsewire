//! Parses RSS/Atom XML bytes into a normalized in-memory representation.
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct FeedMetadata {
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FeedItem {
    pub title: Option<String>,
    pub link: Option<String>,
    pub guid: Option<String>,
    pub published_at_ms: Option<i64>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedFeed {
    pub metadata: FeedMetadata,
    pub items: Vec<FeedItem>,
}

pub fn parse(bytes: &[u8]) -> Result<ParsedFeed, String> {
    let feed = feed_rs::parser::parse(bytes).map_err(|e| format!("feed parse error: {e}"))?;

    let meta = FeedMetadata {
        title: feed.title.map(|t| t.content),
        link: feed.links.first().map(|l| l.href.clone()),
        description: feed.description.map(|d| d.content),
        language: feed.language,
        updated_at_ms: feed.updated.map(|d| to_ms(d)),
    };

    let mut items = Vec::new();
    for e in feed.entries {
        let published = e.published.map(to_ms).or_else(|| e.updated.map(to_ms));
        let category = e.categories.first().map(|c| c.term.clone());
        let summary = e.summary.as_ref().map(|s| s.content.clone());

        let content = e.content.as_ref().and_then(|c| c.body.clone());
        let desc = content.clone().or_else(|| summary.clone());

        items.push(FeedItem {
            title: e.title.map(|t| t.content),
            link: e.links.first().map(|l| l.href.clone()),
            guid: Some(e.id),
            published_at_ms: published,
            category,
            description: desc.clone(),
            summary: summary.or(content),
        });
    }

    Ok(ParsedFeed {
        metadata: meta,
        items,
    })
}

fn to_ms(dt: DateTime<Utc>) -> i64 {
    dt.timestamp_millis()
}
