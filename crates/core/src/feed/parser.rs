//! Parses RSS/Atom XML bytes into a
//! normalized in-memory representation.

use chrono::{
  DateTime,
  Utc
};

#[derive(Debug, Clone)]

pub struct FeedMetadata {
  pub title:         Option<String>,
  pub link:          Option<String>,
  pub description:   Option<String>,
  pub language:      Option<String>,
  pub updated_at_ms: Option<i64>
}

#[derive(Debug, Clone)]

pub struct FeedItem {
  pub title:           Option<String>,
  pub link:            Option<String>,
  pub guid:            Option<String>,
  pub published_at_ms: Option<i64>,
  pub category:        Option<String>,
  pub description:     Option<String>,
  pub summary:         Option<String>
}

#[derive(Debug, Clone)]

pub struct ParsedFeed {
  pub metadata: FeedMetadata,
  pub items:    Vec<FeedItem>
}

pub fn parse(
  bytes: &[u8]
) -> Result<ParsedFeed, String> {
  if bytes.len()
    > crate::infra::http_policy::MAX_RESPONSE_BODY_BYTES
  {
    return Err("feed exceeds maximum response size".into());
  }
  let feed =
    feed_rs::parser::parse(bytes)
      .map_err(|e| {
        format!("feed parse error: {e}")
      })?;

  let meta = FeedMetadata {
    title:         feed
      .title
      .map(|t| t.content),
    link:          feed
      .links
      .first()
      .map(|l| l.href.clone()),
    description:   feed
      .description
      .map(|d| d.content),
    language:      feed.language,
    updated_at_ms: feed
      .updated
      .map(to_ms)
  };

  let mut items = Vec::new();

  for e in feed.entries {
    let published = e
      .published
      .map(to_ms)
      .or_else(|| e.updated.map(to_ms));

    let category = e
      .categories
      .first()
      .map(|c| c.term.clone());

    let summary = e
      .summary
      .as_ref()
      .map(|s| s.content.clone());

    let content = e
      .content
      .as_ref()
      .and_then(|c| c.body.clone());

    let desc = content
      .clone()
      .or_else(|| summary.clone());

    items.push(FeedItem {
      title: e.title.map(|t| t.content),
      link: e
        .links
        .first()
        .map(|l| l.href.clone()),
      guid: Some(e.id),
      published_at_ms: published,
      category,
      description: desc.clone(),
      summary: summary.or(content)
    });
  }

  Ok(ParsedFeed {
    metadata: meta,
    items
  })
}

fn to_ms(dt: DateTime<Utc>) -> i64 {
  dt.timestamp_millis()
}

#[cfg(test)]
mod offline_feed_regressions {
  use super::*;

  #[test]
  fn rss_item_metadata_survives_normalization()
   {
    let rss = br#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>News &amp; Research</title>
    <link>https://example.org/feed</link>
    <description>Daily updates</description>
    <item>
      <title>First article</title>
      <link>https://example.org/first</link>
      <guid isPermaLink="false">feed-item-1</guid>
      <description>First summary</description>
      <pubDate>Fri, 01 Jan 2021 00:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>"#;
    let parsed =
      parse(rss).expect("valid RSS");
    assert_eq!(
      parsed.metadata.title.as_deref(),
      Some("News & Research")
    );
    assert_eq!(parsed.items.len(), 1);
    let item = &parsed.items[0];
    assert_eq!(
      item.title.as_deref(),
      Some("First article")
    );
    assert_eq!(
      item.guid.as_deref(),
      Some("feed-item-1")
    );
    assert_eq!(
      item.link.as_deref(),
      Some("https://example.org/first")
    );
    assert_eq!(
      item.summary.as_deref(),
      Some("First summary")
    );
    assert_eq!(
      item.published_at_ms,
      Some(1_609_459_200_000)
    );
  }

  #[test]
  fn atom_item_uses_updated_when_published_is_missing()
   {
    let atom = br#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Research Feed</title>
  <id>urn:example:feed</id>
  <updated>2020-01-02T00:00:00Z</updated>
  <entry>
    <title>Atom article</title>
    <id>urn:example:article:1</id>
    <updated>2021-01-01T00:00:00Z</updated>
    <summary>Atom summary</summary>
  </entry>
</feed>"#;
    let parsed =
      parse(atom).expect("valid Atom");
    assert_eq!(
      parsed.metadata.title.as_deref(),
      Some("Research Feed")
    );
    assert_eq!(parsed.items.len(), 1);
    let item = &parsed.items[0];
    assert_eq!(
      item.guid.as_deref(),
      Some("urn:example:article:1")
    );
    assert_eq!(
      item.description.as_deref(),
      Some("Atom summary")
    );
    assert_eq!(
      item.published_at_ms,
      Some(1_609_459_200_000)
    );
  }

  #[test]
  fn rejects_invalid_and_empty_feed_documents()
   {
    for malformed in [
      b"".as_slice(),
      b"not an XML feed".as_slice(),
      b"<rss><channel><item>"
        .as_slice()
    ] {
      assert!(
        parse(malformed).is_err(),
        "malformed feed must fail"
      );
    }
  }

  #[test]
  fn decompressed_body_over_eight_mib_is_rejected_before_parsing()
   {
    let over_limit = vec![
      b' ';
      crate::infra::http_policy::MAX_RESPONSE_BODY_BYTES + 1
    ];
    assert_eq!(
      parse(&over_limit)
        .expect_err("over-limit feed"),
      "feed exceeds maximum response \
       size"
    );
  }
}
