// PW-070 operator-documentation contract: Node built-ins only, no npm.
// Local links and promised runtime boundaries must agree with checked-in
// Compose, CLI, config and release guardrails before milestone acceptance.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const read = (path) => readFileSync(resolve(root, path), "utf8");
const files = [
  "README.md",
  "crates/fetcher/README.md",
  "crates/cli/README.md",
  "docs/reference/RELEASE.md",
  "docs/pulsewire/DEPLOYMENT.md",
  "docs/pulsewire/COMPATIBILITY.md",
  "docs/pulsewire/TROUBLESHOOTING.md",
  "docs/pulsewire/SECRETS.md",
  "docs/pulsewire/BACKUP-RESTORE.md",
  "docs/pulsewire/RECOVERY.md",
  "docs/pulsewire/FETCH_SECURITY.md",
  "docs/pulsewire/MONITORING.md",
  "docs/pulsewire/PW061-ACCEPTANCE.md"
];

const linkProblems = [];
let checkedLinks = 0;
for (const file of files) {
  assert.ok(existsSync(resolve(root, file)), "missing operator document " + file);
  const body = read(file);
  for (const match of body.matchAll(/!?\[[^\]\n]*\]\((<[^>\n]*>|[^\s)\n]+)(?:\s+["'][^"']*["'])?\)/g)) {
    let target = match[1].replace(/^<|>$/g, "");
    if (/^(https?:\/\/|mailto:|data:|#)/i.test(target)) continue;
    target = target.split("#")[0].split("?")[0];
    let decoded;
    try { decoded = decodeURIComponent(target); }
    catch { linkProblems.push(file + " bad link encoding: " + target); continue; }
    const path = resolve(root, dirname(file), decoded);
    const rel = relative(root, path);
    if (rel === ".." || rel.startsWith(".." + sep) || isAbsolute(rel) || !existsSync(path)) {
      linkProblems.push(file + " -> " + target + " (resolved " + rel + ")");
    }
    checkedLinks++;
  }
}
assert.deepEqual(linkProblems, [], "broken or escaping local Markdown links");

const readme = read("README.md");
const justfile = read("justfile");
const recipes = new Set([...justfile.matchAll(/^([A-Za-z][\w-]*)\s*:/gm)].map((m) => m[1]));
const claims = [...readme.matchAll(/\x60just ([\w-]+)\x60/g)].map((m) => m[1]);
for (const recipe of claims) {
  assert.ok(recipes.has(recipe), "README claims nonexistent just recipe: " + recipe);
}

const fetcherDocs = read("crates/fetcher/README.md");
const cliDocs = read("crates/cli/README.md");
const cliSource = read("crates/cli/src/main.rs");
const fetcherMain = read("crates/fetcher/src/main.rs");
const serverMain = read("crates/server/src/startup.rs");
assert.match(fetcherDocs, /preserves.{0,150}databases on startup/s, "dev mode must not claim wipe");
assert.doesNotMatch(fetcherDocs, /Dev mode can wipe/i, "obsolete destructive boot claim");
assert.match(fetcherMain, /preserving[\s\\]+database contents on startup/, "fetcher startup preservation source");
assert.match(serverMain, /dev\.reset_on_start is/, "server rejects auto-reset");
for (const body of [cliDocs, cliSource]) {
  for (const requirement of ["PULSEWIRE_DEV_DATA_DIR", "PULSEWIRE_ALLOW_DESTRUCTIVE_DEV_RESET", "--confirm"]) {
    assert.ok(body.includes(requirement), "clean safety gate missing " + requirement);
  }
}
const releaseDocs = read("docs/reference/RELEASE.md");
for (const requirement of ["PW-071", "git push --tags", "owner approves", "rollback"]) {
  assert.ok(releaseDocs.toLowerCase().includes(requirement.toLowerCase()), "release warning missing " + requirement);
}
const secretDocs = read("docs/pulsewire/SECRETS.md");
const secretCode = read("crates/schemas/src/secrets.rs");
const coreLoader = read("crates/core/src/infra/config/loader.rs");
const serverConfig = read("crates/server/src/config.rs");
for (const body of [secretDocs, secretCode, coreLoader, serverConfig]) {
  assert.ok(body.includes("PULSEWIRE_POSTGRES_PASSWORD_FILE") || body.includes("postgres_password("), "missing file-secret support");
}
for (const service of ["fetcher", "server"]) {
  const overlay = read("crates/" + service + "/docker-compose.secrets.yml");
  const baseline = read("crates/" + service + "/docker-compose.yml");
  assert.ok(overlay.includes("PULSEWIRE_POSTGRES_PASSWORD_FILE: /run/secrets/pulsewire_postgres_password"), "missing file-secret env");
  assert.ok(overlay.includes('PULSEWIRE_POSTGRES_PASSWORD: ""'), "must clear direct env credential");
  assert.ok(overlay.includes("PULSEWIRE_POSTGRES_PASSWORD_SOURCE"), "private source must be required");
  assert.ok(baseline.includes("PULSEWIRE_POSTGRES_PASSWORD_FILE"), "baseline must accept file secret");
}
for (const required of ["both", "fail", "Docker", "PW-023"]) {
  assert.ok(secretDocs.includes(required), "secret runbook missing caveat " + required);
}
const compatibility = read("docs/pulsewire/COMPATIBILITY.md");
const deployment = read("docs/pulsewire/DEPLOYMENT.md");
const troubleshoot = read("docs/pulsewire/TROUBLESHOOTING.md");
for (const required of ["PostgreSQL", "SQLite", "x86_64-unknown-linux-musl", "sessionStorage", "PULSEWIRE_DOCKER_NETWORK", "PULSEWIRE_ALLOW_PRIVATE_FEEDS", "PULSEWIRE_DEV_DATA_DIR"]) {
  assert.ok(compatibility.includes(required), "compatibility missing " + required);
}
for (const required of ["PW-023", "PW-032", "PW-071", "BACKUP-RESTORE.md", "TROUBLESHOOTING.md", "COMPATIBILITY.md"]) {
  assert.ok(deployment.includes(required), "deployment preflight missing " + required);
}
for (const required of ["/health", "/ready", "429", "PULSEWIRE_POSTGRES_PASSWORD", "PULSEWIRE_DEV_DATA_DIR", "RECOVERY.md"]) {
  assert.ok(troubleshoot.includes(required), "troubleshooting missing " + required);
}

const serverDocker = read("crates/server/res/docker.toml");
const fetcherDocker = read("crates/fetcher/res/docker.toml");
const serverCompose = read("crates/server/docker-compose.yml");
const fetcherCompose = read("crates/fetcher/docker-compose.yml");
const categories = read("crates/fetcher/res/categories.toml");
const dockerIgnore = read(".dockerignore");
const tuiDefault = read("crates/tui/res/config.toml");
assert.match(serverDocker, /^\[http\][\s\S]*?^host\s*=\s*"0\.0\.0\.0"/m, "server internal bind");
assert.match(serverCompose, /127\.0\.0\.1:8091:8091/, "server Compose host-only binding");
for (const compose of [serverCompose, fetcherCompose]) {
  assert.match(compose, /external:\s*true/, "Compose must not create shared Docker network");
  for (const required of ["PULSEWIRE_DOCKER_NETWORK", "PULSEWIRE_POSTGRES_PASSWORD", "PULSEWIRE_POSTGRES_HOST"]) {
    assert.ok(compose.includes(required), "Compose missing " + required);
  }
}
assert.match(fetcherDocker, /^\[metrics\][\s\S]*?enabled\s*=\s*false/m, "metrics are opt-in");
assert.match(fetcherDocker, /^\[metrics\][\s\S]*?bind\s*=\s*"127\.0\.0\.1:9898"/m, "metrics bind");
assert.doesNotMatch(fetcherCompose, /^\s*ports:\s*$/m, "fetcher must not publish metrics by default");
for (const category of ["youtube", "podcasts"]) {
  assert.match(categories, new RegExp('name\\s*=\\s*"' + category + '"\\s*enabled\\s*=\\s*false'), "disabled " + category);
}
for (const exclude of ["/crates/fetcher/res/feeds/youtube.com", "/crates/fetcher/res/feeds/podcasts"]) {
  assert.ok(dockerIgnore.includes(exclude), "image exclusion missing " + exclude);
}
assert.match(tuiDefault, /auto_login\s*=\s*false/, "no default TUI auto-login");
assert.match(tuiDefault, /password\s*=\s*""/, "no checked-in TUI password");
for (const file of ["crates/server/Dockerfile", "crates/fetcher/Dockerfile"]) {
  const body = read(file);
  assert.match(body, /ARG TARGET=x86_64-unknown-linux-musl/, "container target " + file);
  assert.match(body, /USER 65532:65532/, "non-root container " + file);
}
assert.match(read("rust-toolchain.toml"), /channel\s*=\s*"nightly"/, "local toolchain guidance");
console.log("PW-070 documentation contract passed: " + files.length + " documents, " + checkedLinks + " local links, source-backed defaults, safety and release controls.");
