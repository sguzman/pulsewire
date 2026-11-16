// PW-071: release-readiness inspection and fail-closed manual gate.
// Read-only: this module never runs cargo-release, tags, pushes,
// deploys, changes versions, or contacts a remote service.
// An owner-controlled receipt is an assertion, NOT cryptographic proof.
import { readFileSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const ROADMAP = resolve(ROOT, "docs/pulsewire/ROADMAP.md");
const VALID_STATUS = new Set(["verified complete", "active", "pending", "blocked"]);
const VALID_TYPE = new Set(["major", "minor", "patch"]);
const REQUIRED_IDS = ["PW-023", "PW-032", "PW-071"];
const REQUIRED_RECEIPT_GATES = [
  "pw023_host_exposure",
  "pw032_target_resource_envelope",
  "pw071_host_deployment",
  "backup_restore_target_host",
  "exact_commit_ci",
  "owner_release_authorization"
];

function reject(reason) {
  throw new Error("PW-071 denied: " + reason);
}
function object(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function exactKeys(value, expected, label) {
  if (!object(value) ||
      JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...expected].sort())) {
    reject(label + " contains missing or unexpected fields");
  }
}
export function analyzeRoadmap(source) {
  if (typeof source !== "string") reject("missing canonical roadmap");
  const declared = source.match(/Current accepted weighted verification credit:\s*\*\*(\d+)\/100\*\*/);
  if (!declared) reject("canonical score not found");
  const rows = source.split("\n").filter((line) => /^\|\s*PW-\d{3}\s*\|/.test(line));
  if (rows.length !== 27) reject("frozen roadmap has an unexpected milestone count");
  const seen = new Set();
  let total = 0;
  let accepted = 0;
  const outstanding = [];
  for (const line of rows) {
    const columns = line.split("|").map((x) => x.trim());
    if (columns.length !== 10) reject("malformed milestone table");
    const [id, name, rawWeight, priority, status] = columns.slice(1, 6);
    if (!/^PW-\d{3}$/.test(id) || seen.has(id)) reject("duplicate or malformed milestone id");
    seen.add(id);
    const weight = Number(rawWeight);
    if (!Number.isSafeInteger(weight) || weight < 1 || weight > 20 ||
        !["P0", "P1"].includes(priority) || !VALID_STATUS.has(status)) {
      reject("unknown weight, priority, or status for " + id);
    }
    if (!name) reject("unnamed milestone " + id);
    total += weight;
    if (status === "verified complete") accepted += weight;
    else outstanding.push({ id, priority, status, weight });
  }
  if (total !== 100 || Number(declared[1]) !== accepted) {
    reject("score and accepted milestone weights disagree");
  }
  for (const id of REQUIRED_IDS) if (!seen.has(id)) {
    reject("required milestone missing: " + id);
  }
  return {
    schema: 1,
    kind: "pw071_read_only_release_status",
    score: { accepted, total },
    outstanding,
    all_milestones_verified: outstanding.length === 0,
    external_host_evidence_verified: false,
    release_approved: false,
    actions_performed: [],
    source: "canonical_repository_roadmap"
  };
}
export function validateReceipt(receipt, expectedSha, releaseType) {
  if (!/^[a-f0-9]{40}$/i.test(expectedSha) || !VALID_TYPE.has(releaseType)) {
    reject("invalid exact commit or release type");
  }
  exactKeys(receipt,
    ["schema", "repository", "commit_sha", "release_type", "owner_approved",
     "accepted_gates", "evidence_references", "approval_recorded_at"],
    "owner receipt");
  if (receipt.schema !== 1 || receipt.repository !== "sguzman/pulsewire" ||
      receipt.commit_sha !== expectedSha ||
      receipt.release_type !== releaseType || receipt.owner_approved !== true) {
    reject("owner receipt does not approve this exact repository, commit and release type");
  }
  if (typeof receipt.approval_recorded_at !== "string" ||
      !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(receipt.approval_recorded_at)) {
    reject("owner receipt has no UTC approval timestamp");
  }
  exactKeys(receipt.accepted_gates, REQUIRED_RECEIPT_GATES, "accepted gates");
  exactKeys(receipt.evidence_references, REQUIRED_RECEIPT_GATES, "evidence references");
  for (const gate of REQUIRED_RECEIPT_GATES) {
    if (receipt.accepted_gates[gate] !== true) reject("gate not accepted: " + gate);
    const evidence = receipt.evidence_references[gate];
    if (typeof evidence !== "string" || evidence.length < 12 || evidence.length > 512 ||
        !/^https:\/\/|^sha256:/.test(evidence)) {
      reject("missing reviewed evidence reference: " + gate);
    }
  }
  // An offline tool cannot verify the purported human signature,
  // remote CI state, network tests, or document provenance.
  return {
    declaration_format_valid: true,
    cryptographic_owner_approval_verified: false,
    live_ci_independently_verified: false,
    deployed: false
  };
}
function git(...args) {
  return execFileSync("git", args, {
    cwd: ROOT, encoding: "utf8",
    timeout: 5000, stdio: ["ignore", "pipe", "ignore"]
  }).trim();
}
function approvalOutsideRepository(filePath) {
  if (typeof filePath !== "string" || !isAbsolute(filePath)) {
    reject("PULSEWIRE_RELEASE_APPROVAL_FILE must be an absolute private path");
  }
  const path = resolve(filePath);
  const rel = relative(ROOT, path);
  if (rel === "" || (rel !== ".." && !rel.startsWith(".." + sep) &&
                     !isAbsolute(rel))) {
    reject("approval receipt must be stored outside the repository");
  }
  const stat = statSync(path);
  if (!stat.isFile() || (stat.mode & 0o077) !== 0 || stat.size > 8192 ||
      stat.size < 40) {
    reject("owner receipt must be a private, bounded regular file");
  }
  return path;
}
function enforce(roadmap, releaseType) {
  if (!VALID_TYPE.has(releaseType)) reject("unsupported release type");
  if (!roadmap.all_milestones_verified) {
    reject("milestones remain unaccepted: " +
      roadmap.outstanding.map((entry) => entry.id).join(", "));
  }
  if (git("rev-parse", "--abbrev-ref", "HEAD") !== "main") {
    reject("only a checked-out main branch can be considered");
  }
  if (git("status", "--porcelain").length !== 0) {
    reject("working tree is not clean");
  }
  const sha = git("rev-parse", "HEAD");
  if (!/^[a-f0-9]{40}$/.test(sha)) reject("cannot resolve exact commit");
  const file = approvalOutsideRepository(process.env.PULSEWIRE_RELEASE_APPROVAL_FILE);
  let receipt;
  try {
    receipt = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    reject("cannot read valid private owner approval receipt");
  }
  validateReceipt(receipt, sha, releaseType);
  // Passing this manual declaration check is still not remote CI,
  // deployment or cryptographically verified owner approval.
  // The Just recipes deliberately DO NOT execute release operations.
  return { ...roadmap, release_type: releaseType,
    reviewed_commit_claim: sha,
    owner_receipt_format_valid: true,
    release_execution_performed: false,
    release_approved: false,
    actions_performed: [] };
}

function cli(args) {
  if (args.length === 1 && args[0] === "--status") {
    const report = analyzeRoadmap(readFileSync(ROADMAP, "utf8"));
    process.stdout.write(JSON.stringify(report, null, 2) + "\n");
    return;
  }
  if (args.length === 3 && args[0] === "--enforce" &&
      args[1] === "--type" && VALID_TYPE.has(args[2])) {
    const report = analyzeRoadmap(readFileSync(ROADMAP, "utf8"));
    process.stdout.write(JSON.stringify(enforce(report, args[2]), null, 2) + "\n");
    return;
  }
  reject("usage: --status OR --enforce --type major|minor|patch");
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    cli(process.argv.slice(2));
  } catch (error) {
    const message = error instanceof Error && error.message.startsWith("PW-071 denied:")
      ? error.message : "PW-071 denied: unable to verify release prerequisites";
    process.stderr.write(message + "\n");
    process.exitCode = 2;
  }
}
