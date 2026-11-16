// PW-032: no-npm offline envelope contract tests. No host access.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const source = new URL("../scripts/pw032-envelope.mjs", import.meta.url);
const fixture = JSON.parse(readFileSync(
  new URL("./fixtures/pw032-resource-synthetic.json", import.meta.url), "utf8"
));
function run(snapshot) {
  return spawnSync(process.execPath,
    [fileURLToPath(source), "--input", "-"], {
      input: JSON.stringify(snapshot),
      encoding: "utf8",
      timeout: 10000
    });
}
function clone(input) {
  return JSON.parse(JSON.stringify(input));
}
function reject(label, mutate) {
  const input = clone(fixture);
  mutate(input);
  const result = run(input);
  assert.equal(result.status, 1, label + " must fail closed");
  assert.match(result.stderr, /PW-032 invalid measurement/, label);
  assert.equal(result.stdout, "", label + " should never emit a partial report");
}

const passed = run(fixture);
assert.equal(passed.status, 0, passed.stderr);
const report = JSON.parse(passed.stdout);
assert.equal(report.kind, "pw032_resource_envelope");
assert.equal(report.source_claim, "synthetic_ci");
assert.equal(report.window.sample_count, 4);
assert.equal(report.window.elapsed_seconds, 180);
assert.equal(report.catalog_feed_count, 20);
assert.equal(report.cpu_container_percent.max, 40);
assert.equal(report.cpu_container_percent.mean, 23.75);
assert.equal(report.memory_bytes.max, 150000000);
assert.equal(report.database_bytes.change, 3000000);
assert.equal(report.backlog_due.change, -23);
assert.equal(report.feed_actions.completed_delta, 50);
assert.equal(report.http_latency.observed_action_count, 50);
assert.equal(report.http_latency.p95_histogram_upper_ms, 500);
assert.deepEqual(report.acceptance, {
  pw032_weighted_points: 0,
  target_host_verified: false,
  sustainable_capacity_proven: false,
  deployment_approved: false
});
assert.ok(report.warnings.some((x) => /Synthetic inputs/.test(x)));
assert.ok(!passed.stdout.includes("environment"));
assert.ok(!passed.stdout.includes("password"));

reject("missing provenance", (d) => { delete d.source; });
reject("unapproved source type", (d) => { d.source = "verified_target_host"; });
reject("unapproved catalog", (d) => { d.catalog.youtube = 99; });
reject("extra environment field", (d) => { d.samples[0].environment = "private"; });
reject("too few samples", (d) => { d.samples = d.samples.slice(0, 2); });
reject("non-monotonic timestamp", (d) => { d.samples[2].timestamp_ms = d.samples[1].timestamp_ms; });
reject("non-finite CPU", (d) => { d.samples[1].cpu_percent = "NaN"; });
reject("negative memory", (d) => { d.samples[1].memory_bytes = -5; });
reject("fractional backlog", (d) => { d.samples[1].backlog_due = 2.2; });
reject("counter reset", (d) => { d.samples[3].completed_actions_total = 2; });
reject("histogram bucket inversion", (d) => { d.samples[2].http_latency_buckets["250"] = 1; });
reject("histogram counter reset", (d) => { d.samples[3].http_latency_buckets["25"] = 0; });
reject("missing latency bucket", (d) => { delete d.samples[2].http_latency_buckets["+Inf"]; });
reject("invalid commit", (d) => { d.commit = "main"; });

const noObservations = clone(fixture);
for (const sample of noObservations.samples) {
  for (const bound of Object.keys(sample.http_latency_buckets)) {
    sample.http_latency_buckets[bound] = 0;
  }
}
const emptyResult = run(noObservations);
assert.equal(emptyResult.status, 0, emptyResult.stderr);
const emptyReport = JSON.parse(emptyResult.stdout);
assert.equal(emptyReport.http_latency.observed_action_count, 0);
assert.equal(emptyReport.http_latency.p95_histogram_upper_ms, null);
assert.ok(emptyReport.warnings.some((s) => /No new HTTP observations/.test(s)));

const unbounded = clone(fixture);
unbounded.samples.at(-1).http_latency_buckets["10000"] = 40;
const overflow = run(unbounded);
assert.equal(overflow.status, 0, overflow.stderr);
const overflowReport = JSON.parse(overflow.stdout);
assert.equal(overflowReport.http_latency.p95_histogram_upper_ms, null);
assert.ok(overflowReport.warnings.some((x) => /upper bound unknown/.test(x)));

const rising = clone(fixture);
rising.samples.at(-1).backlog_due = 41;
const risingReport = JSON.parse(run(rising).stdout);
assert.ok(risingReport.warnings.some((s) => /backlog increased/.test(s)));

const claimedOperator = clone(fixture);
claimedOperator.source = "operator_recorded";
const claimedReport = JSON.parse(run(claimedOperator).stdout);
assert.equal(claimedReport.acceptance.target_host_verified, false);
assert.equal(claimedReport.acceptance.pw032_weighted_points, 0);
console.log("PW-032 synthetic resource envelope passed: CPU, memory, DB growth, backlog, bounded HTTP p95, provenance and fail-closed inputs");
