// PW-032: offline raw Docker/Prometheus -> sanitized observation tests.
// No npm, docker socket, real feed, secrets, network or DB is accessed.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const normalizer = fileURLToPath(new URL(
  "../scripts/pw032-ci-normalize.mjs", import.meta.url
));
const reporter = fileURLToPath(new URL(
  "../scripts/pw032-envelope.mjs", import.meta.url
));
const bounds = ["25", "50", "100", "250", "500",
  "1000", "2000", "5000", "10000", "+Inf"];
function prom(count) {
  const done = [
    '# HELP pulsewire_up Service is running.',
    '# TYPE pulsewire_up gauge',
    'pulsewire_up 1',
    'pulsewire_feed_actions_total{action="head",outcome="ok"} 0',
    'pulsewire_feed_actions_total{action="head",outcome="err"} 0',
    'pulsewire_feed_actions_total{action="get",outcome="ok"} ' + count,
    'pulsewire_feed_actions_total{action="get",outcome="err"} 0'
  ];
  for (const bound of bounds) {
    const bucket = bound === "25" ? 0
      : bound === "50" ? 0
      : bound === "100" ? Math.floor(count / 2) : count;
    done.push('pulsewire_http_latency_ms_bucket{' +
      'action="get",domain="ci-rss",le="' + bound +
      '"} ' + bucket);
  }
  return done.join("\n") + "\n";
}
const fixture = {
  schema: 1,
  source: "synthetic_ci",
  commit: "a".repeat(40),
  catalog: { news: 1, government: 0, journals: 0 },
  samples: [0, 1, 2].map((n) => ({
    timestamp_ms: 1000000 + n * 2000,
    docker_stats: {
      CPUPerc: ["2.4%", "8.8%", "4.2%"][n],
      MemUsage: ["12.5MiB / 512MiB", "13.0MiB / 512MiB",
                 "14.0MiB / 512MiB"][n],
      Container: "never-export-ci-container-id",
      Name: "private-inventory-placeholder",
      Env: "never-export-ci-secret"
    },
    database_bytes: 10000000 + n * 4096,
    backlog_due: 3 - n,
    metrics_text: prom(n)
  }))
};
function call(script, input) {
  return spawnSync(process.execPath, [script, "--input", "-"], {
    input: JSON.stringify(input),
    encoding: "utf8",
    timeout: 10000
  });
}
function clone() {
  return structuredClone(fixture);
}
function denied(name, fn) {
  const input = clone();
  fn(input);
  const run = call(normalizer, input);
  assert.equal(run.status, 1, name + ": " + run.stderr);
  assert.match(run.stderr, /PW-032 invalid disposable observation/);
  assert.equal(run.stdout, "");
  assert.ok(!run.stderr.includes("never-export-ci-secret"));
}
const normalized = call(normalizer, fixture);
assert.equal(normalized.status, 0, normalized.stderr);
assert.ok(!normalized.stdout.includes("never-export"));
assert.ok(!normalized.stdout.includes("ci-rss"));
const safe = JSON.parse(normalized.stdout);
assert.deepEqual(safe.catalog, fixture.catalog);
assert.deepEqual(safe.samples.map((x) => x.memory_bytes),
  [13107200, 13631488, 14680064]);
assert.deepEqual(safe.samples.map((x) => x.cpu_percent),
  [2.4, 8.8, 4.2]);
assert.deepEqual(safe.samples.map((x) => x.completed_actions_total),
  [0, 1, 2]);
assert.deepEqual(safe.samples.map((x) => x.http_latency_buckets["+Inf"]),
  [0, 1, 2]);
const analyzed = call(reporter, safe);
assert.equal(analyzed.status, 0, analyzed.stderr);
const report = JSON.parse(analyzed.stdout);
assert.equal(report.feed_actions.completed_delta, 2);
assert.equal(report.window.sample_count, 3);
assert.equal(report.database_bytes.change, 8192);
assert.equal(report.http_latency.p95_histogram_upper_ms, 250);
assert.equal(report.acceptance.target_host_verified, false);
assert.equal(report.acceptance.pw032_weighted_points, 0);
assert.equal(report.acceptance.deployment_approved, false);
assert.ok(report.warnings.some((s) => s.includes("Synthetic inputs")));

denied("missing source", (d) => { delete d.source; });
denied("host provenance rejected", (d) => { d.source = "operator_recorded"; });
denied("unknown field in sample", (d) => { d.samples[1].postgres_password = "never-export-ci-secret"; });
denied("unknown catalog category", (d) => { d.catalog.podcasts = 1; });
denied("bad CPU", (d) => { d.samples[0].docker_stats.CPUPerc = "N/A"; });
denied("bad memory", (d) => { d.samples[0].docker_stats.MemUsage = "unknown"; });
denied("memory beyond container limit", (d) => {
  d.samples[0].docker_stats.MemUsage = "750MiB / 512MiB";
});
denied("invalid metrics body", (d) => { d.samples[0].metrics_text = "secret"; });
denied("missing action", (d) => {
  d.samples[0].metrics_text = d.samples[0].metrics_text.replace(
    /pulsewire_feed_actions_total\{action="get",outcome="ok"\} 0\n/, ""
  );
});
denied("missing histogram bucket", (d) => {
  d.samples[1].metrics_text = d.samples[1].metrics_text.replace(
    /pulsewire_http_latency_ms_bucket\{action="get",domain="ci-rss",le="250"\} 1\n/, ""
  );
});
denied("duplicate action", (d) => {
  d.samples[1].metrics_text +=
    'pulsewire_feed_actions_total{action="get",outcome="ok"} 1\n';
});
denied("inverted histogram", (d) => {
  d.samples[1].metrics_text = d.samples[1].metrics_text.replace(
    /le="500"\} 1/, 'le="500"} 0'
  );
});
denied("bad database count", (d) => { d.samples[1].database_bytes = -1; });
console.log("PW-032 disposable CI normalization passed: real-format Docker stats, PostgreSQL counts, private Prometheus histograms and no-leak guards");
