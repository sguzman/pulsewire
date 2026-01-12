#[derive(Debug, Clone)]
pub(crate) struct Histogram {
  pub(crate) buckets: Vec<u64>,
  pub(crate) sum:     u64,
  pub(crate) count:   u64
}

impl Histogram {
  pub(crate) fn new(
    bucket_len: usize
  ) -> Self {
    Self {
      buckets: vec![0; bucket_len + 1],
      sum:     0,
      count:   0
    }
  }
}

pub(crate) fn record_histogram(
  hist: &mut Histogram,
  value_ms: u64,
  buckets: &[u64]
) {
  hist.sum += value_ms;
  hist.count += 1;

  let mut idx = 0;

  while idx < buckets.len() {
    if value_ms <= buckets[idx] {
      hist.buckets[idx] += 1;
      return;
    }

    idx += 1;
  }

  hist.buckets[idx] += 1;
}
