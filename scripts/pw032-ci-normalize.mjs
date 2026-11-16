// PW-032: normalize disposable CI observation text into the offline envelope.
// No Docker, database, HTTP, credential, or system calls. Never emit raw inputs.
import { readFileSync } from "node:fs";

const BOUNDS = ["25", "50", "100", "250", "500", "1000",
                "2000", "5000", "10000", "+Inf"];
const UNITS = {
  B: 1, kB: 1000, KB: 1000, KiB: 1024, MB: 1e6,
  MiB: 1024 ** 2, GB: 1e9, GiB: 1024 ** 3,
  TB: 1e12, TiB: 1024 ** 4
};
const ACTIONS = ["head", "get"];
const OUTCOMES = ["ok", "err"];

function invalid(reason) {
  throw new Error("PW-032 invalid disposable observation: " + reason);
}
function keys(value, wanted, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    invalid(label + " must be an object");
  }
  if (JSON.stringify(Object.keys(value).sort()) !==
      JSON.stringify([...wanted].sort())) {
    invalid(label + " has missing or unexpected fields");
  }
}
function count(value, label) {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    invalid(label + " must be a nonnegative safe integer");
  }
}
function stats(statsObject) {
  if (!statsObject || typeof statsObject !== "object" ||
      Array.isArray(statsObject)) invalid("Docker stats object missing");
  // Docker's JSON formatter includes unrelated container identifiers,
  // disk/network details etc. Never copy those fields to our output.
  const cpu = statsObject.CPUPerc;
  const mem = statsObject.MemUsage;
  if (typeof cpu !== "string" || !/^\d+(?:\.\d+)?%$/.test(cpu)) {
    invalid("invalid Docker CPU percentage format");
  }
  const percent = Number(cpu.slice(0, -1));
  if (!Number.isFinite(percent)) invalid("nonfinite CPU percentage");
  if (typeof mem !== "string") invalid("invalid Docker memory format");
  const match = mem.match(
    /^(\d+(?:\.\d+)?)\s*(B|kB|KB|KiB|MB|MiB|GB|GiB|TB|TiB)\s*\/\s*(\d+(?:\.\d+)?)\s*(B|kB|KB|KiB|MB|MiB|GB|GiB|TB|TiB)$/
  );
  if (!match) invalid("unrecognized Docker memory usage unit");
  const used = Number(match[1]) * UNITS[match[2]];
  const limit = Number(match[3]) * UNITS[match[4]];
  if (!Number.isSafeInteger(Math.round(used)) || used < 0 ||
      !Number.isFinite(limit) || limit < 1 ||
      used > limit * 1.02) {
    invalid("invalid Docker memory usage quantity");
  }
  return { cpu_percent: percent, memory_bytes: Math.round(used) };
}
function label(labels, field) {
  const found = [...labels.matchAll(/(?:^|,)([a-z_]+)="([^"]*)"/g)]
    .filter((pair) => pair[1] === field);
  return found.length === 1 ? found[0][2] : null;
}
function parseMetrics(raw) {
  if (typeof raw !== "string" || raw.length > 300000 || !raw.includes("pulsewire_up 1")) {
    invalid("missing or oversized running Prometheus metrics");
  }
  const actions = new Map();
  const histograms = new Map();
  for (const line of raw.split("\n")) {
    const action = line.match(/^pulsewire_feed_actions_total\{(.+)\} ([0-9]+)$/);
    if (action) {
      const which = label(action[1], "action");
      const outcome = label(action[1], "outcome");
      if (!ACTIONS.includes(which) || !OUTCOMES.includes(outcome)) {
        invalid("unexpected action/outcome metric");
      }
      const key = which + ":" + outcome;
      if (actions.has(key)) invalid("duplicate feed-action series");
      const value = Number(action[2]);
      count(value, "feed-action counter");
      actions.set(key, value);
      continue;
    }
    const bucket = line.match(/^pulsewire_http_latency_ms_bucket\{(.+)\} ([0-9]+)$/);
    if (!bucket) continue;
    const which = label(bucket[1], "action");
    const domain = label(bucket[1], "domain");
    const le = label(bucket[1], "le");
    if (!ACTIONS.includes(which) || domain === null || domain.length === 0 ||
        !BOUNDS.includes(le)) {
      invalid("unexpected HTTP histogram label");
    }
    // Preserve series identity only in memory; never emit domain labels.
    const key = which + "\x00" + domain;
    if (!histograms.has(key)) histograms.set(key, new Map());
    const series = histograms.get(key);
    if (series.has(le)) invalid("duplicate HTTP histogram bucket");
    const value = Number(bucket[2]);
    count(value, "histogram count");
    series.set(le, value);
  }
  for (const action of ACTIONS) {
    for (const outcome of OUTCOMES) {
      if (!actions.has(action + ":" + outcome)) {
        invalid("missing HEAD/GET action counter");
      }
    }
  }
  const totals = Object.fromEntries(BOUNDS.map((bound) => [bound, 0]));
  for (const series of histograms.values()) {
    if (series.size !== BOUNDS.length) {
      invalid("incomplete HTTP histogram series");
    }
    let previous = 0;
    for (const bound of BOUNDS) {
      const value = series.get(bound);
      if (value === undefined || value < previous) {
        invalid("HTTP histogram bucket is not cumulative");
      }
      totals[bound] += value;
      count(totals[bound], "aggregate histogram counter");
      previous = value;
    }
  }
  const done = [...actions.values()].reduce((sum, value) => sum + value, 0);
  count(done, "completed HEAD/GET total");
  return { done, totals };
}
function normalize(input) {
  keys(input, ["schema", "source", "commit", "catalog", "samples"], "root");
  if (input.schema !== 1 || input.source !== "synthetic_ci") {
    invalid("only a disposable synthetic CI source is supported");
  }
  if (typeof input.commit !== "string" || !/^[0-9a-f]{40}$/i.test(input.commit)) {
    invalid("missing checked-out source SHA");
  }
  keys(input.catalog, ["news", "government", "journals"], "catalog");
  for (const category of Object.values(input.catalog)) count(category, "catalog count");
  if ((input.catalog.news + input.catalog.government +
       input.catalog.journals) === 0) invalid("no fixture feeds");
  if (!Array.isArray(input.samples) || input.samples.length < 3 ||
      input.samples.length > 60) invalid("require 3 to 60 CI samples");
  const result = [];
  for (const sample of input.samples) {
    keys(sample, ["timestamp_ms", "docker_stats", "database_bytes",
                  "backlog_due", "metrics_text"], "sample");
    for (const key of ["timestamp_ms", "database_bytes", "backlog_due"]) {
      count(sample[key], key);
    }
    const mem = stats(sample.docker_stats);
    const metrics = parseMetrics(sample.metrics_text);
    result.push({
      timestamp_ms: sample.timestamp_ms,
      ...mem,
      database_bytes: sample.database_bytes,
      backlog_due: sample.backlog_due,
      completed_actions_total: metrics.done,
      http_latency_buckets: metrics.totals
    });
  }
  return {
    schema: 1, source: "synthetic_ci",
    commit: input.commit, catalog: input.catalog,
    samples: result
  };
}

try {
  if (process.argv.length !== 4 || process.argv[2] !== "--input") {
    invalid("usage: node scripts/pw032-ci-normalize.mjs --input FILE|-");
  }
  const text = readFileSync(process.argv[3] === "-" ? 0 : process.argv[3], "utf8");
  if (Buffer.byteLength(text) > 1200000) invalid("observation input too large");
  const result = normalize(JSON.parse(text));
  process.stdout.write(JSON.stringify(result, null, 2) + "\n");
} catch (error) {
  // Do not echo input, Docker container metadata, URLs, labels or secrets.
  const safe = error instanceof Error &&
    error.message.startsWith("PW-032 invalid disposable observation:");
  process.stderr.write((safe ? error.message :
    "PW-032 invalid disposable observation input") + "\n");
  process.exitCode = 1;
}
