// PW-071: no-npm, offline release/readiness safety acceptance.
// Never runs a release, tag, push, deployment or network request.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { analyzeRoadmap, approvalOutsideRepository, validateReceipt } from
  "../scripts/pw071-release-preflight.mjs";

const roadmap = readFileSync(
  new URL("../docs/pulsewire/ROADMAP.md", import.meta.url), "utf8"
);
const justfile = readFileSync(new URL("../justfile", import.meta.url), "utf8");
const releasePath = fileURLToPath(new URL(
  "../scripts/pw071-release-preflight.mjs", import.meta.url
));
const expectedSha = "a".repeat(40);

function denied(name, callback, pattern = /PW-071 denied/) {
  assert.throws(callback, pattern, name);
}
const report = analyzeRoadmap(roadmap);
assert.equal(report.schema, 1);
assert.equal(report.score.accepted, 93);
assert.equal(report.score.total, 100);
assert.equal(report.all_milestones_verified, false);
assert.deepEqual(report.outstanding.map((x) => x.id),
  ["PW-023", "PW-032", "PW-071"]);
assert.deepEqual(report.outstanding.map((x) => x.weight), [3, 2, 2]);
assert.equal(report.external_host_evidence_verified, false);
assert.equal(report.release_approved, false);
assert.deepEqual(report.actions_performed, []);

const status = spawnSync(process.execPath, [releasePath, "--status"], {
  encoding: "utf8", timeout: 10000
});
assert.equal(status.status, 0, status.stderr);
assert.equal(status.stderr, "");
const reported = JSON.parse(status.stdout);
assert.deepEqual(reported, report);
const blocked = spawnSync(process.execPath,
  [releasePath, "--enforce", "--type", "patch"], {
    encoding: "utf8", timeout: 10000
  }
);
assert.equal(blocked.status, 2);
assert.match(blocked.stderr, /PW-071 denied: milestones remain unaccepted/);
for (const id of ["PW-023", "PW-032", "PW-071"]) {
  assert.ok(blocked.stderr.includes(id));
}
assert.equal(blocked.stdout, "");
const wrongType = spawnSync(process.execPath,
  [releasePath, "--enforce", "--type", "push"], { encoding: "utf8" });
assert.equal(wrongType.status, 2);
assert.match(wrongType.stderr, /PW-071 denied/);

denied("mismatched accepted weight",
  () => analyzeRoadmap(roadmap.replace(
    "Current accepted weighted verification credit: **93/100**",
    "Current accepted weighted verification credit: **94/100**"
  )));
const row = roadmap.split("\n").find((line) => line.startsWith("| PW-023 |"));
assert.ok(row);
denied("duplicate milestone",
  () => analyzeRoadmap(roadmap.replace(row, row + "\n" + row)));
denied("missing milestone", () => analyzeRoadmap(roadmap.replace(row, "")));
denied("invalid milestone status",
  () => analyzeRoadmap(roadmap.replace(
    "| PW-023 | 🔑 Secrets and network exposure | 3 | P0 | active |",
    "| PW-023 | 🔑 Secrets and network exposure | 3 | P0 | complete |"
  )));
const future = roadmap
  .replace("| PW-023 | 🔑 Secrets and network exposure | 3 | P0 | active |",
           "| PW-023 | 🔑 Secrets and network exposure | 3 | P0 | verified complete |")
  .replace("| PW-032 | 📉 Realistic resource envelope | 2 | P1 | pending |",
           "| PW-032 | 📉 Realistic resource envelope | 2 | P1 | verified complete |")
  .replace("| PW-071 | 🏁 Verified acceptance and release | 2 | P1 | pending |",
           "| PW-071 | 🏁 Verified acceptance and release | 2 | P1 | verified complete |")
  .replace("Current accepted weighted verification credit: **93/100**",
           "Current accepted weighted verification credit: **100/100**");
const simulated = analyzeRoadmap(future);
assert.equal(simulated.all_milestones_verified, true);
assert.equal(simulated.score.accepted, 100);
assert.equal(simulated.release_approved, false);
assert.equal(simulated.external_host_evidence_verified, false);

const gates = [
  "pw023_host_exposure", "pw032_target_resource_envelope",
  "pw071_host_deployment", "backup_restore_target_host",
  "exact_commit_ci", "owner_release_authorization"
];
const receipt = {
  schema: 1, repository: "sguzman/pulsewire",
  commit_sha: expectedSha, release_type: "patch",
  owner_approved: true,
  approval_recorded_at: "2026-11-16T00:00:00Z",
  accepted_gates: Object.fromEntries(gates.map((key) => [key, true])),
  evidence_references: Object.fromEntries(gates.map(
    (key) => [key, "https://example.invalid/review/" + key]
  ))
};
const parsed = validateReceipt(receipt, expectedSha, "patch");
assert.equal(parsed.declaration_format_valid, true);
assert.equal(parsed.cryptographic_owner_approval_verified, false);
assert.equal(parsed.live_ci_independently_verified, false);
assert.equal(parsed.deployed, false);
function clone(value) {
  return structuredClone(value);
}
// Evidence references are untrusted declarations, but their format must
// never let malformed hashes, broken URLs or embedded credentials pass.
const hashedReceipt = clone(receipt);
hashedReceipt.evidence_references.exact_commit_ci = "sha256:" + "b".repeat(64);
assert.equal(validateReceipt(hashedReceipt, expectedSha, "patch").declaration_format_valid, true);
for (const bad of [
  "sha256:not-a-real-digest",
  "sha256:" + "x".repeat(64),
  "https://",
  "https://username:password@example.invalid/review",
  "https://example.invalid/review#fragment",
  "http://example.invalid/review"
]) {
  denied("invalid evidence reference: " + bad, () => {
    const x = clone(receipt);
    x.evidence_references.exact_commit_ci = bad;
    validateReceipt(x, expectedSha, "patch");
  });
}
for (const bad of ["2026-02-29T00:00:00Z", "2026-13-01T00:00:00Z",
                   "2026-11-16T24:00:00Z", "not-a-timestamp"]) {
  denied("invalid UTC approval timestamp: " + bad, () => {
    const x = clone(receipt);
    x.approval_recorded_at = bad;
    validateReceipt(x, expectedSha, "patch");
  });
}

// This fixture is a private synthetic temp file, not an operator receipt.
const temporary = mkdtempSync(join(tmpdir(), "pw071-approval-"));
try {
  const privateReceipt = join(temporary, "private.json");
  writeFileSync(privateReceipt, "synthetic-only-".repeat(8), { mode: 0o600 });
  assert.equal(approvalOutsideRepository(privateReceipt), privateReceipt);
  const fileAlias = join(temporary, "file-alias.json");
  symlinkSync(privateReceipt, fileAlias);
  denied("direct symlink to private receipt",
    () => approvalOutsideRepository(fileAlias));
  const parentAlias = join(temporary, "folder-alias");
  symlinkSync(temporary, parentAlias, "dir");
  denied("symlinked parent directory",
    () => approvalOutsideRepository(join(parentAlias, "private.json")));
  denied("receipt inside repository",
    () => approvalOutsideRepository(fileURLToPath(new URL("../README.md", import.meta.url))));
} finally {
  rmSync(temporary, { recursive: true, force: true });
}

denied("owner approval absent", () => {
  const x = clone(receipt); x.owner_approved = false;
  validateReceipt(x, expectedSha, "patch");
});
denied("wrong release type",
  () => validateReceipt(receipt, expectedSha, "minor"));
denied("wrong commit",
  () => validateReceipt(receipt, "b".repeat(40), "patch"));
denied("missing host backup evidence", () => {
  const x = clone(receipt); x.accepted_gates.backup_restore_target_host = false;
  validateReceipt(x, expectedSha, "patch");
});
denied("missing CI evidence", () => {
  const x = clone(receipt); x.evidence_references.exact_commit_ci = "";
  validateReceipt(x, expectedSha, "patch");
});
denied("unexpected receipt keys", () => {
  const x = clone(receipt); x.secret = "never-print-me";
  validateReceipt(x, expectedSha, "patch");
});

// Inspect the actual source recipes: no destructive release operations.
// This is more reliable than invoking a dangerous command for testing.
assert.ok(justfile.includes("release-status:"));
for (const type of ["major", "minor", "patch"]) {
  const recipe = justfile.match(new RegExp(
    "^" + type + ":\\n((?:\\t[^\\n]*\\n)+)", "m"
  ));
  assert.ok(recipe, "release recipe missing " + type);
  assert.ok(recipe[1].includes("--enforce --type " + type));
  assert.doesNotMatch(recipe[1],
    /cargo\s+release|--execute|git\s+push|git\s+tag|docker|kubectl|helm/);
}
console.log("PW-071 release preflight passed: canonical 93/100 blockers, fail-closed Just recipes, strict synthetic receipts/path validation, no tag/push");
