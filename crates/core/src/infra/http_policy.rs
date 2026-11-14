//! Outbound feed URL and redirect
//! policy. This checks URL syntax and
//! known unsafe literal hosts; DNS
//! resolution still
//! requires network-side egress
//! controls.

use std::net::{
  IpAddr,
  Ipv4Addr,
  Ipv6Addr
};

use reqwest::Url;

/// Maximum decompressed response body
/// kept by the feed fetcher (8 MiB).
pub const MAX_RESPONSE_BODY_BYTES:
  usize = 8 * 1024 * 1024;

pub const MAX_REDIRECTS: usize = 5;

/// Explicit trusted-operator opt-in for
/// private feed networks. Strict egress
/// is the default.
pub fn allow_private_feed_networks()
-> bool {
  std::env::var(
    "PULSEWIRE_ALLOW_PRIVATE_FEEDS"
  )
  .ok()
  .as_deref()
    == Some("1")
}

fn blocked_ipv4(ip: Ipv4Addr) -> bool {
  let a = ip.octets();
  ip.is_private()
    || ip.is_loopback()
    || ip.is_link_local()
    || ip.is_multicast()
    || ip.is_broadcast()
    || ip.is_unspecified()
    || a[0] == 0
    || a[0] >= 224
    || (a[0] == 100
      && (64..=127).contains(&a[1]))
    || (a[0] == 192
      && a[1] == 0
      && a[2] == 0)
    || (a[0] == 198
      && (18..=19).contains(&a[1]))
    || (a[0] == 192
      && a[1] == 0
      && a[2] == 2)
    || (a[0] == 198
      && a[1] == 51
      && a[2] == 100)
    || (a[0] == 203
      && a[1] == 0
      && a[2] == 113)
    || (a[0] == 192
      && a[1] == 88
      && a[2] == 99)
}

fn blocked_ipv6(ip: Ipv6Addr) -> bool {
  ip.is_loopback()
    || ip.is_unspecified()
    || ip.is_multicast()
    || ip.is_unique_local()
    || ip.is_unicast_link_local()
    || ip.to_ipv4_mapped().is_some_and(blocked_ipv4)
    // Exclude non-global and special-use v6 ranges.
    || (ip.segments()[0] & 0xe000) != 0x2000
    || (ip.segments()[0] == 0x2001 && ip.segments()[1] <= 0x01ff)
    || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8)
    || ip.segments()[0] == 0x2002
}

/// Shared by literal URL and
/// connection-time DNS validation.
pub(crate) fn blocked_ip(
  ip: IpAddr
) -> bool {
  match ip {
    | IpAddr::V4(a) => blocked_ipv4(a),
    | IpAddr::V6(a) => blocked_ipv6(a)
  }
}

/// Only conventional, credential-free
/// HTTP(S) sources are permitted.
pub fn validate_url(
  parsed: &Url,
  allow_local_test: bool
) -> Result<(), &'static str> {
  if !matches!(
    parsed.scheme(),
    "http" | "https"
  ) {
    return Err(
      "unsupported URL scheme"
    );
  }
  if !parsed.username().is_empty()
    || parsed.password().is_some()
  {
    return Err("credentials in URL");
  }
  if parsed.fragment().is_some() {
    return Err(
      "URL fragment not allowed"
    );
  }
  let host = parsed
    .host_str()
    .ok_or("URL host required")?;
  if host.is_empty() {
    return Err("URL host required");
  }
  if !allow_local_test {
    let host_lower =
      host.to_ascii_lowercase();
    if host_lower == "localhost"
      || host_lower
        .ends_with(".localhost")
      || host_lower.ends_with(".local")
      || host_lower
        .ends_with(".internal")
      || host_lower
        .ends_with(".home.arpa")
    {
      return Err(
        "local hostname blocked"
      );
    }
    let literal = host
      .trim_start_matches('[')
      .trim_end_matches(']');
    if let Ok(ip) =
      literal.parse::<IpAddr>()
    {
      let forbidden = match ip {
        | IpAddr::V4(a) => {
          blocked_ipv4(a)
        }
        | IpAddr::V6(a) => {
          blocked_ipv6(a)
        }
      };
      if forbidden {
        return Err(
          "non-public IP blocked"
        );
      }
    }
  }
  Ok(())
}

pub fn parse_source_url(
  raw: &str,
  allow_local_test: bool
) -> Result<Url, &'static str> {
  if raw.len() > 8192
    || raw.trim() != raw
  {
    return Err(
      "invalid URL length or \
       whitespace"
    );
  }
  let parsed = Url::parse(raw)
    .map_err(|_| "malformed URL")?;
  validate_url(
    &parsed,
    allow_local_test
  )?;
  Ok(parsed)
}

pub fn redirect_allowed(
  previous: &Url,
  next: &Url,
  allow_local_test: bool
) -> bool {
  if validate_url(
    next,
    allow_local_test
  )
  .is_err()
  {
    return false;
  }
  if previous.host_str()
    != next.host_str()
  {
    // Never forward source cookies or
    // custom headers to another host.
    return false;
  }
  if previous.scheme() == "https"
    && next.scheme() != "https"
  {
    return false;
  }
  if previous.origin() == next.origin()
  {
    return true;
  }
  // Permit only a conventional
  // same-host HTTP-to-HTTPS upgrade.
  previous.scheme() == "http"
    && next.scheme() == "https"
    && previous.port_or_known_default()
      == Some(80)
    && next.port_or_known_default()
      == Some(443)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rejects_unsafe_source_urls() {
    for url in [
      "file:///etc/passwd",
      "ftp://example.org/feed",
      "https://reader:secret@example.org/feed",
      "http://localhost/admin",
      "http://127.0.0.1/admin",
      "http://169.254.169.254/latest/meta-data/",
      "http://192.168.1.1/",
      "http://10.1.2.3/",
      "http://[::1]/",
      "http://[::ffff:127.0.0.1]/",
      "http://site.local/feed",
      "https://example.org/feed#section",
      "not a URL",
      " https://example.org/feed"
    ] {
      assert!(
        parse_source_url(url, false).is_err(),
        "should reject {url}"
      );
    }
    assert!(
      parse_source_url(
        "https://example.org/news?x=1",
        false
      )
      .is_ok()
    );
  }

  #[test]
  fn redirect_restricts_host_and_downgrade()
   {
    let base = Url::parse(
      "https://example.org/feed"
    )
    .expect("base");
    for target in [
      "http://example.org/feed",
      "https://other.example.org/feed",
      "https://127.0.0.1/",
      "https://example.org:4443/feed"
    ] {
      let next = Url::parse(target)
        .expect("target");
      assert!(
        !redirect_allowed(
          &base, &next, false
        ),
        "should deny {target}"
      );
    }
    let same = Url::parse(
      "https://example.org/next"
    )
    .expect("same host");
    assert!(redirect_allowed(
      &base, &same, false
    ));
    let insecure = Url::parse(
      "http://example.org/feed"
    )
    .expect("http");
    assert!(redirect_allowed(
      &insecure, &base, false
    ));
  }
}
