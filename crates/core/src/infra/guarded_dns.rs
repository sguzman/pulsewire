//! Connection-time DNS policy for
//! outbound feed requests.
//! The addresses returned here are the
//! addresses Reqwest connects
//! to; a separate hostname preflight
//! would allow DNS rebinding.

use std::io;
use std::net::SocketAddr;

use reqwest::dns::{
  Addrs,
  Name,
  Resolve,
  Resolving
};

use super::http_policy::blocked_ip;

pub struct GuardedDns {
  allow_private: bool
}

impl GuardedDns {
  pub fn new(
    allow_private: bool
  ) -> Self {
    Self {
      allow_private
    }
  }
}

fn check_destinations(
  addrs: &[SocketAddr],
  allow_private: bool
) -> io::Result<()> {
  if addrs.is_empty() {
    return Err(io::Error::new(
      io::ErrorKind::NotFound,
      "no DNS destinations for feed"
    ));
  }
  if !allow_private
    && addrs
      .iter()
      .any(|addr| blocked_ip(addr.ip()))
  {
    return Err(io::Error::new(
      io::ErrorKind::PermissionDenied,
      "non-public DNS destination for \
       feed"
    ));
  }
  Ok(())
}

impl Resolve for GuardedDns {
  fn resolve(
    &self,
    name: Name
  ) -> Resolving {
    let host = name.as_str().to_owned();
    let allow_private =
      self.allow_private;
    Box::pin(async move {
      let addresses: Vec<SocketAddr> =
        tokio::net::lookup_host((
          host.as_str(),
          0
        ))
        .await?
        .collect();
      check_destinations(
        &addresses,
        allow_private
      )?;
      Ok::<
        Addrs,
        Box<
          dyn std::error::Error
            + Send
            + Sync
        >
      >(Box::new(
        addresses.into_iter()
      ))
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn denies_mixed_dns_and_metadata_addresses()
   {
    let public = ["1.1.1.1:80"
      .parse()
      .expect("public IP")];
    let rebinding = [
      "1.1.1.1:80"
        .parse()
        .expect("public"),
      "10.0.0.53:80"
        .parse()
        .expect("private")
    ];
    let metadata = ["169.254.169.254:\
                     80"
      .parse()
      .expect("metadata")];
    let docs = ["[2001:db8::1]:80"
      .parse()
      .expect("special-use IPv6")];
    assert!(
      check_destinations(
        &public, false
      )
      .is_ok()
    );
    assert!(
      check_destinations(
        &rebinding, false
      )
      .is_err()
    );
    assert!(
      check_destinations(
        &metadata, false
      )
      .is_err()
    );
    assert!(
      check_destinations(&docs, false)
        .is_err()
    );
    assert!(
      check_destinations(&[], false)
        .is_err()
    );
    assert!(
      check_destinations(
        &rebinding, true
      )
      .is_ok()
    );
  }

  #[tokio::test]
  async fn blocks_localhost_at_actual_resolver()
   {
    let name: Name = "localhost"
      .parse()
      .expect("DNS name");
    let result = GuardedDns::new(false)
      .resolve(name)
      .await;
    assert!(result.is_err());
  }
}
