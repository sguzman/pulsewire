use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{
  OwnedSemaphorePermit,
  RwLock,
  Semaphore
};

use crate::domain::model::AppConfig;

#[derive(Clone)]

pub struct ConcurrencyGuards {
  global:  Option<Arc<Semaphore>>,
  domains: Arc<
    RwLock<
      HashMap<String, Arc<Semaphore>>
    >
  >,
  cfg:     Arc<AppConfig>
}

impl ConcurrencyGuards {
  pub fn new(
    cfg: Arc<AppConfig>
  ) -> Self {
    let mut per: HashMap<
      String,
      Arc<Semaphore>
    > = HashMap::new();

    for (domain, dcfg) in &cfg.domains {
      per.insert(
        domain.clone(),
        Arc::new(Semaphore::new(
          dcfg.max_concurrent_requests
        ))
      );
    }

    // The old serial loop had a default
    // 64-feed window. With concurrent
    // category loops that limit must be
    // global rather than per-category.
    let limit = global_limit(
      cfg
        .global_max_concurrent_requests
    );
    let global = Some(Arc::new(
      Semaphore::new(limit)
    ));

    Self {
      global,
      domains: Arc::new(RwLock::new(
        per
      )),
      cfg
    }
  }

  pub async fn permit(
    &self,
    domain: &str
  ) -> PermitPair {
    let maybe = {
      self
        .domains
        .read()
        .await
        .get(domain)
        .cloned()
    };

    let sem = if let Some(s) = maybe {
      s
    } else {
      let limit = self
        .cfg
        .domains
        .get(domain)
        .map(|d| {
          d.max_concurrent_requests
        })
        .unwrap_or(1);

      let mut guard =
        self.domains.write().await;

      guard
        .entry(domain.to_string())
        .or_insert_with(|| {
          Arc::new(Semaphore::new(
            limit
          ))
        })
        .clone()
    };

    PermitPair::acquire(
      self.global.clone(),
      sem
    )
    .await
  }
}

pub struct PermitPair {
  _g: Option<OwnedSemaphorePermit>,
  _d: OwnedSemaphorePermit
}

impl PermitPair {
  async fn acquire(
    global: Option<Arc<Semaphore>>,
    domain: Arc<Semaphore>
  ) -> Self {
    // A busy domain must not hold an
    // idle global request slot.
    let d = domain
      .acquire_owned()
      .await
      .expect(
        "domain semaphore closed"
      );

    let g = match global {
      | Some(s) => {
        Some(
          s.acquire_owned()
            .await
            .expect(
              "global semaphore closed"
            )
        )
      }
      | None => None
    };

    Self {
      _g: g, _d: d
    }
  }
}

fn global_limit(
  configured: Option<usize>
) -> usize {
  configured.unwrap_or(64).max(1)
}

#[cfg(test)]
mod global_budget_tests {
  use super::*;

  #[test]
  fn default_and_zero_limits_are_bounded()
   {
    assert_eq!(global_limit(None), 64);
    assert_eq!(
      global_limit(Some(0)),
      1
    );
    assert_eq!(
      global_limit(Some(7)),
      7
    );
  }

  #[tokio::test]
  async fn one_global_slot_is_shared_by_domains()
   {
    let global =
      Arc::new(Semaphore::new(1));
    let news =
      Arc::new(Semaphore::new(1));
    let science =
      Arc::new(Semaphore::new(1));

    let first = PermitPair::acquire(
      Some(global.clone()),
      news
    )
    .await;
    assert_eq!(
      global.available_permits(),
      0
    );
    assert!(
      global.try_acquire().is_err()
    );
    drop(first);
    let second = PermitPair::acquire(
      Some(global.clone()),
      science
    )
    .await;
    assert_eq!(
      global.available_permits(),
      0
    );
    drop(second);
    assert_eq!(
      global.available_permits(),
      1
    );
  }
  #[tokio::test]
  async fn busy_domain_never_occupies_global_budget()
   {
    let global =
      Arc::new(Semaphore::new(1));
    let busy =
      Arc::new(Semaphore::new(1));
    let other =
      Arc::new(Semaphore::new(1));
    let held = busy
      .clone()
      .acquire_owned()
      .await
      .expect("hold busy domain");

    let blocked = PermitPair::acquire(
      Some(global.clone()),
      busy
    );
    let ready = PermitPair::acquire(
      Some(global.clone()),
      other
    );
    tokio::pin!(blocked);
    tokio::pin!(ready);

    let advanced = tokio::time::timeout(
      std::time::Duration::from_secs(1),
      async {
        tokio::select! {
          biased;
          _ = &mut blocked => false,
          _ = &mut ready => true,
        }
      }
    )
    .await
    .expect("unrelated domain must run");

    assert!(advanced);
    assert_eq!(
      global.available_permits(),
      1
    );
    drop(held);
  }

}
