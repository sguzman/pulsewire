use std::{collections::HashMap, sync::Arc};

use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};

use crate::domain::model::AppConfig;

#[derive(Clone)]
pub struct ConcurrencyGuards {
    global: Option<Arc<Semaphore>>,
    domains: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    cfg: Arc<AppConfig>,
}

impl ConcurrencyGuards {
    pub fn new(cfg: Arc<AppConfig>) -> Self {
        let mut per: HashMap<String, Arc<Semaphore>> = HashMap::new();
        for (domain, dcfg) in &cfg.domains {
            per.insert(
                domain.clone(),
                Arc::new(Semaphore::new(dcfg.max_concurrent_requests)),
            );
        }
        let global = cfg
            .global_max_concurrent_requests
            .map(|n| Arc::new(Semaphore::new(n)));
        Self {
            global,
            domains: Arc::new(RwLock::new(per)),
            cfg,
        }
    }

    pub async fn permit(&self, domain: &str) -> PermitPair {
        let maybe = { self.domains.read().await.get(domain).cloned() };
        let sem = if let Some(s) = maybe {
            s
        } else {
            let limit = self
                .cfg
                .domains
                .get(domain)
                .map(|d| d.max_concurrent_requests)
                .unwrap_or(1);
            let mut guard = self.domains.write().await;
            guard
                .entry(domain.to_string())
                .or_insert_with(|| Arc::new(Semaphore::new(limit)))
                .clone()
        };
        PermitPair::acquire(self.global.clone(), sem).await
    }
}

pub struct PermitPair {
    _g: Option<OwnedSemaphorePermit>,
    _d: OwnedSemaphorePermit,
}

impl PermitPair {
    async fn acquire(global: Option<Arc<Semaphore>>, domain: Arc<Semaphore>) -> Self {
        let g = match global {
            Some(s) => Some(s.acquire_owned().await.expect("global semaphore closed")),
            None => None,
        };
        let d = domain
            .acquire_owned()
            .await
            .expect("domain semaphore closed");
        Self { _g: g, _d: d }
    }
}
