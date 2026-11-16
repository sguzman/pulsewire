// PW-032: deterministic, no-npm summary of explicitly supplied measurements.
// Offline only. Never samples Docker, SQL, networks, files outside --input,
// credentials, or actual household services.
import { readFileSync } from "node:fs";

const BOUNDS = ["25", "50", "100", "250", "500", "1000",
                "2000", "5000", "10000", "+Inf"];
const SAMPLE_KEYS = [
  "timestamp_ms", "cpu_percent", "memory_bytes", "database_bytes",
  "backlog_due", "completed_actions_total", "http_latency_buckets"
];
const ROOT_KEYS = ["schema", "source", "commit", "catalog", "samples"];
const CATALOG_KEYS = ["news", "government", "journals"];

function fail(message) {
  throw new Error("PW-032 invalid measurement: " + message);
}
function object(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(label + " must be an object");
  }
}
function exactKeys(value, required, label) {
  object(value, label);
  const actual = Object.keys(value).sort();
  if (JSON.stringify(actual) !== JSON.stringify([...required].sort())) {
    fail(label + " has missing or unexpected fields");
  }
}
function finite(value, label) {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    fail(label + " must be finite and nonnegative");
  }
}
function integer(value, label) {
  finite(value, label);
  if (!Number.isSafeInteger(value)) fail(label + " must be a safe integer");
}
function percentile(sortedValues, percentileRank) {
  return sortedValues[Math.ceil(sortedValues.length * percentileRank) - 1];
}
function round(value, digits = 3) {
  return Number(value.toFixed(digits));
}
function report(input) {
  exactKeys(input, ROOT_KEYS, "root");
  if (input.schema !== 1) fail("unsupported schema");
  if (!["synthetic_ci", "operator_recorded"].includes(input.source)) {
    fail("unsupported source; measurements must identify provenance");
  }
  if (typeof input.commit !== "string" || !/^[a-f0-9]{40}$/i.test(input.commit)) {
    fail("commit must be a 40-character Git SHA");
  }
  exactKeys(input.catalog, CATALOG_KEYS, "catalog");
  for (const category of CATALOG_KEYS) integer(input.catalog[category], category + " count");
  const feedCount = CATALOG_KEYS.reduce((sum, key) => sum + input.catalog[key], 0);
  if (feedCount < 1) fail("empty slim catalog");
  if (!Array.isArray(input.samples) || input.samples.length < 3) {
    fail("at least three ordered time samples are required");
  }
  if (input.samples.length > 20000) fail("sample limit exceeded");
  let prev;
  for (const [idx, sample] of input.samples.entries()) {
    const label = "samples[" + idx + "]";
    exactKeys(sample, SAMPLE_KEYS, label);
    for (const metric of SAMPLE_KEYS.slice(0, -1)) {
      if (metric === "cpu_percent") finite(sample[metric], label + "." + metric);
      else integer(sample[metric], label + "." + metric);
    }
    if (prev && sample.timestamp_ms <= prev.timestamp_ms) {
      fail("timestamps must be strictly increasing");
    }
    exactKeys(sample.http_latency_buckets, BOUNDS, label + ".http_latency_buckets");
    let priorBucket = 0;
    for (const bound of BOUNDS) {
      const count = sample.http_latency_buckets[bound];
      integer(count, label + ".http_latency_buckets." + bound);
      if (count < priorBucket) fail("histogram buckets must be cumulative");
      if (prev && count < prev.http_latency_buckets[bound]) {
        fail("histogram counters cannot decrease across samples");
      }
      priorBucket = count;
    }
    if (prev && sample.completed_actions_total < prev.completed_actions_total) {
      fail("completed_actions_total cannot decrease");
    }
    prev = sample;
  }
  const first = input.samples[0];
  const last = input.samples.at(-1);
  const duration = (last.timestamp_ms - first.timestamp_ms) / 1000;
  if (duration <= 0) fail("non-positive measurement window");
  const cpu = input.samples.map((x) => x.cpu_percent).sort((a, b) => a - b);
  const mem = input.samples.map((x) => x.memory_bytes).sort((a, b) => a - b);
  const backlog = input.samples.map((x) => x.backlog_due);
  const histogramDelta = Object.fromEntries(BOUNDS.map(
    (key) => [key, last.http_latency_buckets[key] - first.http_latency_buckets[key]]
  ));
  let previousDelta = 0;
  for (const bound of BOUNDS) {
    if (histogramDelta[bound] < previousDelta) {
      fail("histogram interval deltas must be cumulative; incompatible samples");
    }
    previousDelta = histogramDelta[bound];
  }
  const count = histogramDelta["+Inf"];
  let p95 = null;
  if (count) {
    const threshold = Math.ceil(count * 0.95);
    for (const bucket of BOUNDS) {
      if (histogramDelta[bucket] >= threshold) {
        p95 = bucket === "+Inf" ? null : Number(bucket);
        break;
      }
    }
  }
  const actionDelta = last.completed_actions_total - first.completed_actions_total;
  const steps = input.samples.slice(1).map(
    (s, idx) => (s.timestamp_ms - input.samples[idx].timestamp_ms) / 1000
  );
  const warnings = [];
  if (input.source === "synthetic_ci") {
    warnings.push("Synthetic inputs are not target-host measurements.");
  }
  if (count === 0) warnings.push("No new HTTP observations in the measurement window.");
  if (count && p95 === null) {
    warnings.push("HTTP p95 exceeds the largest finite histogram bucket; upper bound unknown.");
  }
  if (last.backlog_due > first.backlog_due) {
    warnings.push("Observed due backlog increased over the measurement window.");
  }
  if (Math.max(...steps) > 2 * Math.min(...steps)) {
    warnings.push("Uneven sample intervals may distort sampled CPU and memory percentiles.");
  }
  return {
    schema: 1,
    kind: "pw032_resource_envelope",
    source_claim: input.source,
    commit_claim: input.commit,
    profile: "slim-news-government-journals",
    catalog_counts: input.catalog,
    catalog_feed_count: feedCount,
    window: {
      sample_count: input.samples.length,
      elapsed_seconds: round(duration),
      min_sample_interval_seconds: round(Math.min(...steps)),
      max_sample_interval_seconds: round(Math.max(...steps))
    },
    cpu_container_percent: {
      mean: round(cpu.reduce((sum, x) => sum + x, 0) / cpu.length),
      p95_sampled: percentile(cpu, 0.95),
      max: cpu.at(-1)
    },
    memory_bytes: {
      p95_sampled: percentile(mem, 0.95),
      max: mem.at(-1),
      first: first.memory_bytes,
      last: last.memory_bytes
    },
    database_bytes: {
      first: first.database_bytes,
      last: last.database_bytes,
      change: last.database_bytes - first.database_bytes
    },
    backlog_due: {
      first: first.backlog_due,
      last: last.backlog_due,
      peak: Math.max(...backlog),
      change: last.backlog_due - first.backlog_due
    },
    feed_actions: {
      completed_delta: actionDelta,
      per_minute: round(actionDelta / duration * 60)
    },
    http_latency: {
      observed_action_count: count,
      p95_histogram_upper_ms: p95,
      finite_bucket_limit_ms: 10000,
      interpretation: "Estimated upper histogram bucket for combined HEAD/GET samples, not exact p95."
    },
    warnings,
    acceptance: {
      pw032_weighted_points: 0,
      target_host_verified: false,
      sustainable_capacity_proven: false,
      deployment_approved: false
    }
  };
}

try {
  if (process.argv.length !== 4 || process.argv[2] !== "--input") {
    fail("usage: node scripts/pw032-envelope.mjs --input FILE|-");
  }
  const path = process.argv[3];
  const raw = readFileSync(path === "-" ? 0 : path, "utf8");
  if (Buffer.byteLength(raw) > 3_000_000) fail("input exceeds 3 MB");
  const result = report(JSON.parse(raw));
  process.stdout.write(JSON.stringify(result, null, 2) + "\n");
} catch (error) {
  // Do not echo unknown input values, source paths or raw JSON.
  const known = error instanceof Error && error.message.startsWith("PW-032 invalid measurement:");
  process.stderr.write((known ? error.message : "PW-032 invalid measurement input") + "\n");
  process.exitCode = 1;
}
