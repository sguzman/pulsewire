// PW-060: dependency-free Chrome smoke against embedded reader assets and a
// small mock of the documented HTTP contract. No npm or installs.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../crates/server/res/reader/", import.meta.url));
const html = readFileSync(join(root, "index.html"), "utf8");
const js = readFileSync(join(root, "app.js"), "utf8");
const css = readFileSync(join(root, "styles.css"), "utf8");
const csp = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; base-uri 'none'; form-action 'none'; object-src 'none'; frame-ancestors 'none'";
const item = { id: 7, feed_id: "news.test", title: "A test story", link: "https://example.org/story", published_at_ms: 1700000000000, is_read: false };
const fakeDetail = { ...item, summary: "<img src=x onerror=alert(1)>This must be text", description: "backup" };
const feed = { id: "news.test", url: "https://example.org/rss", domain: "example.org", category: "news", base_poll_seconds: 300, tags: [] };
let subscribed = true, favored = true, member = true;
const harness = "(async () => {\n  const $ = (id) => document.getElementById(id);\n  const check = (ok, label) => { if (!ok) throw new Error(label); };\n  async function until(predicate, label) {\n    for (let i = 0; i < 120; i++) {\n      if (predicate()) return;\n      await new Promise((resolve) => setTimeout(resolve, 50));\n    }\n    throw new Error(\"Timed out: \" + label);\n  }\n  function verdict(result, message) {\n    document.documentElement.dataset.smoke = result;\n    const marker = document.createElement(\"pre\");\n    marker.id = \"smoke-verdict\";\n    marker.textContent = result + \": \" + message;\n    document.body.append(marker);\n  }\n  try {\n    check(!$(\"login-view\").hidden, \"initial login visible\");\n    $(\"username\").value = \"reader\";\n    $(\"password\").value = \"safe-password\";\n    $(\"login-form\").requestSubmit();\n    await until(() => !$(\"reader-app\").hidden && document.querySelectorAll(\".entry-card\").length === 1, \"login and entries\");\n    check($(\"subscription-count\").textContent === \"(1)\", \"subscriptions loaded\");\n    check($(\"favorite-count\").textContent === \"(1)\", \"favorites loaded\");\n    document.querySelector(\".entry-card\").click();\n    await until(() => $(\"detail-body\").textContent.includes(\"This must be text\") && $(\"read-toggle\").textContent === \"Mark unread\", \"detail and mark read\");\n    check(!$(\"detail-body\").querySelector(\"img\"), \"feed markup not executed\");\n    check($(\"detail-body\").textContent.includes(\"<img\"), \"feed markup rendered as text\");\n    check($(\"detail-body\").querySelector(\"a\").rel.includes(\"noopener\"), \"external links hardened\");\n    $(\"theme-select\").value = \"dark\";\n    $(\"theme-select\").dispatchEvent(new Event(\"change\", { bubbles: true }));\n    check(document.documentElement.dataset.theme === \"dark\", \"dark theme\");\n    $(\"theme-select\").value = \"system\";\n    $(\"theme-select\").dispatchEvent(new Event(\"change\", { bubbles: true }));\n    check(!document.documentElement.hasAttribute(\"data-theme\"), \"system theme\");\n    $(\"search-input\").value = \"story\";\n    $(\"search-form\").requestSubmit();\n    await until(() => document.querySelectorAll(\".entry-card\").length === 1 && $(\"page-caption\").textContent === \"1 shown\", \"search results\");\n    $(\"sources-button\").click();\n    check($(\"source-dialog\").open, \"catalog dialog opens\");\n    check($(\"source-results\").textContent.includes(\"example.org\"), \"catalog search renders\");\n    const action = (label) => Array.from($(\"source-results\").querySelectorAll(\"button\")).find((b) => b.textContent === label);\n    check(action(\"Unsubscribe\"), \"unsubscribe action available\");\n    action(\"Unsubscribe\").click();\n    await until(() => $(\"subscription-count\").textContent === \"(0)\", \"unsubscribe\");\n    action(\"+ Subscribe\").click();\n    await until(() => $(\"subscription-count\").textContent === \"(1)\", \"resubscribe\");\n    action(\"★ Unfavorite\").click();\n    await until(() => $(\"favorite-count\").textContent === \"(0)\", \"unfavorite\");\n    action(\"☆ Favorite\").click();\n    await until(() => $(\"favorite-count\").textContent === \"(1)\", \"refavorite\");\n    $(\"source-dialog\").close();\n    document.activeElement.blur();\n    document.dispatchEvent(new KeyboardEvent(\"keydown\", { key: \"/\", bubbles: true }));\n    check(document.activeElement === $(\"search-input\"), \"slash shortcut focuses search\");\n    $(\"folder-button\").click();\n    check($(\"folder-dialog\").open && $(\"folder-manager\").textContent.includes(\"Saved\"), \"folder manager\");\n    $(\"folder-name\").value = \"Work\";\n    $(\"new-folder-form\").requestSubmit();\n    await until(() => $(\"folder-list\").textContent.includes(\"Work\"), \"create folder\");\n    $(\"folder-dialog\").close();\n    $(\"search-input\").value = \"force-error\";\n    $(\"search-form\").requestSubmit();\n    await until(() => $(\"notice\").textContent.includes(\"Mock failure\"), \"visible API failure\");\n    $(\"search-input\").value = \"story\";\n    $(\"search-form\").requestSubmit();\n    await until(() => $(\"notice\").hidden && $(\"page-caption\").textContent === \"1 shown\", \"recover after API error\");\n    $(\"folder-list\").querySelector(\"button\").click();\n    await until(() => $(\"stream-title\").textContent === \"Saved\" && $(\"page-caption\").textContent === \"1 shown\", \"open folder\");\n    $(\"sources-button\").click();\n    check(action(\"Remove from folder\"), \"folder membership controls available\");\n    action(\"Remove from folder\").click();\n    await until(() => action(\"Add to folder\"), \"remove feed from folder\");\n    action(\"Add to folder\").click();\n    await until(() => action(\"Remove from folder\"), \"add feed to folder\");\n    $(\"source-dialog\").close();\n    if (matchMedia(\"(max-width: 620px)\").matches) {\n      check(getComputedStyle($(\"menu-button\")).display !== \"none\", \"mobile menu available\");\n      check(getComputedStyle($(\"detail-pane\")).display === \"none\", \"mobile detail collapsed\");\n    }\n    verdict(\"pass\", \"login, subscription/favorite/folder mutations, errors, safe detail, search, keyboard, mobile\");\n  } catch (error) {\n    verdict(\"fail\", String(error.stack || error));\n  }\n})();";
function json(res, status, data) {
  res.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" });
  res.end(JSON.stringify(data));
}
const server = createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  res.setHeader("content-security-policy", csp);
  res.setHeader("x-content-type-options", "nosniff");
  if (url.pathname === "/reader") { res.setHeader("content-type", "text/html; charset=utf-8"); res.end(html.replace("</head>", '<script src="/test-harness.js" defer></script></head>')); return; }
  if (url.pathname === "/reader/app.js") { res.setHeader("content-type", "text/javascript"); res.end(js); return; }
  if (url.pathname === "/reader/styles.css") { res.setHeader("content-type", "text/css"); res.end(css); return; }
  if (url.pathname === "/test-harness.js") { res.setHeader("content-type", "text/javascript"); res.end(harness); return; }
  if (url.pathname === "/v1/auth/login") { json(res, 200, { token: "smoke-token", token_type: "bearer", expires_in: 3600 }); return; }
  if (req.headers.authorization !== "Bearer smoke-token") { json(res, 401, { error: { code: "unauthorized", message: "unauthorized" } }); return; }
  if (url.pathname === "/v1/feeds") { json(res, 200, [feed]); return; }
  if (url.pathname === "/v1/subscriptions" && req.method === "POST") { subscribed = true; res.writeHead(201); res.end(); return; }
  if (url.pathname === "/v1/subscriptions/news.test" && req.method === "DELETE") { subscribed = false; res.writeHead(204); res.end(); return; }
  if (url.pathname === "/v1/subscriptions") { json(res, 200, subscribed ? [{ feed_id: feed.id }] : []); return; }
  if (url.pathname === "/v1/favorites" && req.method === "POST") { favored = true; res.writeHead(201); res.end(); return; }
  if (url.pathname === "/v1/favorites/news.test" && req.method === "DELETE") { favored = false; res.writeHead(204); res.end(); return; }
  if (url.pathname === "/v1/favorites") { json(res, 200, favored ? [feed] : []); return; }
  if (url.pathname === "/v1/folders" && req.method === "POST") { json(res, 200, { id: 2, name: "Work" }); return; }
  if (url.pathname === "/v1/folders") { json(res, 200, [{ id: 1, name: "Saved" }]); return; }
  if (url.pathname === "/v1/folders/1/feeds" && req.method === "POST") { member = true; res.writeHead(201); res.end(); return; }
  if (url.pathname === "/v1/folders/1/feeds/news.test" && req.method === "DELETE") { member = false; res.writeHead(204); res.end(); return; }
  if (url.pathname === "/v1/folders/1/feeds") { json(res, 200, member ? [{ feed_id: feed.id }] : []); return; }
  if (url.pathname === "/v1/entries/search" && url.searchParams.get("q") === "force-error") { json(res, 503, { error: { code: "unavailable", message: "Mock failure" } }); return; }
  if (url.pathname === "/v1/entries/7") { json(res, 200, fakeDetail); return; }
  if (url.pathname === "/v1/entries/7/read") { res.writeHead(204); res.end(); return; }
  if (url.pathname.startsWith("/v1/entries") || url.pathname.includes("/entries")) {
    json(res, 200, { items: [item], next_offset: 1, next_cursor: 7, since: null }); return;
  }
  json(res, 404, { error: { code: "not_found", message: "Mock endpoint not found: " + url.pathname } });
});
const chrome = process.env.CHROME_BIN || "google-chrome";
const profile = mkdtempSync(join(tmpdir(), "pulsewire-reader-smoke-"));
try {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const origin = "http://127.0.0.1:" + server.address().port + "/reader";
  for (const width of [1280, 440]) {
    const output = await new Promise((resolve, reject) => {
      const child = spawn(chrome, [
        "--headless", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
        "--disable-background-networking", "--no-first-run", "--no-default-browser-check",
        "--user-data-dir=" + join(profile, String(width)),
        "--window-size=" + width + ",850",
        "--virtual-time-budget=16000", "--dump-dom", origin
      ], { stdio: ["ignore", "pipe", "pipe"] });
      let stdout = "", stderr = "";
      child.stdout.on("data", (data) => { stdout += data; });
      child.stderr.on("data", (data) => { stderr += data; });
      child.on("error", reject);
      child.on("close", (code) => {
        if (code !== 0) reject(new Error("Chrome exited " + code + ": " + stderr.slice(-1200)));
        else resolve(stdout);
      });
    });
    const verdict = output.match(/<pre id="smoke-verdict">([^<]*)<\/pre>/);
    assert.match(output, /<html[^>]*data-smoke="pass"/, "Browser " + width + ": " + (verdict?.[1] || output.slice(-2000)));
    console.log("Reader Chromium smoke passed at width " + width + "px");
  }
} finally {
  server.close();
  rmSync(profile, { recursive: true, force: true });
}
