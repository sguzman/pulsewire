// PW-060: disposable live-browser acceptance against a real Pulsewire server.
// Uses node:http as a loopback HTML-injection proxy so Chromium exercises the
// actual Rust /reader assets and /v1 APIs without npm or any third-party JS.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer, request } from "node:http";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const upstream = new URL(process.env.PULSEWIRE_READER_E2E_URL || "");
const password = process.env.PULSEWIRE_READER_E2E_PASSWORD;
const username = process.env.PULSEWIRE_READER_E2E_USERNAME || "owner";
assert.equal(upstream.protocol, "http:", "Only the disposable loopback HTTP fixture is supported");
assert.equal(upstream.hostname, "127.0.0.1", "Never connect a browser test to a non-loopback host");
assert.ok(password && password.length >= 12, "Missing disposable test account password");
const harness = "(async () => {\n  const $ = (id) => document.getElementById(id);\n  const check = (ok, message) => { if (!ok) throw new Error(message); };\n  async function until(predicate, label) {\n    for (let i = 0; i < 180; i++) {\n      if (predicate()) return;\n      await new Promise((resolve) => setTimeout(resolve, 70));\n    }\n    throw new Error(\"Timed out: \" + label + \"; notice=\" + $(\"notice\").textContent);\n  }\n  function result(value, message) {\n    document.documentElement.dataset.smoke = value;\n    const marker = document.createElement(\"pre\");\n    marker.id = \"live-smoke-result\";\n    marker.textContent = value + \": \" + message;\n    document.body.append(marker);\n  }\n  const action = (label) => Array.from($(\"source-results\").querySelectorAll(\"button\"))\n    .find((button) => button.textContent === label);\n  function searchSource() {\n    $(\"sources-button\").click();\n    $(\"source-search\").value = \"pulsewire-ci-real-rss\";\n    $(\"source-search\").dispatchEvent(new Event(\"input\", { bubbles: true }));\n  }\n  try {\n    check(!$(\"login-view\").hidden, \"initial login form visible\");\n    check(!document.documentElement.hasAttribute(\"data-theme\"), \"appearance starts in System\");\n    $(\"username\").value = __PW060_USER__;\n    $(\"password\").value = __PW060_PASS__;\n    $(\"login-form\").requestSubmit();\n    await until(() => !$(\"reader-app\").hidden &&\n      Array.from(document.querySelectorAll(\".entry-card\")).some((card) => card.textContent.includes(\"Pulsewire CI Story One\")), \"real login and fetched entries\");\n    check(document.querySelectorAll(\".entry-card\").length >= 2, \"real fetcher entries loaded\");\n\n    const isMobile = matchMedia(\"(max-width: 620px)\").matches;\n    check(isMobile ? getComputedStyle($(\"menu-button\")).display !== \"none\" : getComputedStyle($(\"menu-button\")).display === \"none\", \"responsive menu\");\n    const story = Array.from(document.querySelectorAll(\".entry-card\")).find((card) => card.textContent.includes(\"Pulsewire CI Story One\"));\n    story.click();\n    await until(() => $(\"detail-body\").textContent.includes(\"The first CI story was fetched and parsed.\") && $(\"read-toggle\").textContent === \"Mark unread\", \"real article detail and read POST\");\n    check($(\"detail-body\").querySelector(\"h2\").textContent === \"Pulsewire CI Story One\", \"real article title\");\n    check($(\"detail-body\").querySelector(\".article-link\").href.includes(\"/story-one\"), \"real original link\");\n    if (isMobile) {\n      check(getComputedStyle($(\"detail-pane\")).display !== \"none\", \"mobile detail opens\");\n    }\n    $(\"read-toggle\").click();\n    await until(() => $(\"read-toggle\").textContent === \"Mark read\", \"restore unread via real DELETE\");\n    if (isMobile) {\n      $(\"back-button\").click();\n      check(getComputedStyle($(\"detail-pane\")).display === \"none\", \"mobile back closes detail\");\n      $(\"menu-button\").click();\n      check(!$(\"sidebar-shade\").hidden, \"mobile source drawer opens\");\n      $(\"sidebar-shade\").click();\n      check($(\"sidebar-shade\").hidden, \"mobile source drawer closes\");\n      $(\"logout-button\").click();\n      await until(() => !$(\"login-view\").hidden && $(\"reader-app\").hidden, \"mobile logout\");\n      result(\"pass\", \"real login, fetcher entries, detail/read/unread, mobile navigation, logout\");\n      return;\n    }\n\n    $(\"theme-select\").value = \"dark\";\n    $(\"theme-select\").dispatchEvent(new Event(\"change\", { bubbles: true }));\n    check(document.documentElement.dataset.theme === \"dark\", \"explicit dark theme\");\n    $(\"theme-select\").value = \"light\";\n    $(\"theme-select\").dispatchEvent(new Event(\"change\", { bubbles: true }));\n    check(document.documentElement.dataset.theme === \"light\", \"explicit light theme\");\n    $(\"theme-select\").value = \"system\";\n    $(\"theme-select\").dispatchEvent(new Event(\"change\", { bubbles: true }));\n    check(!document.documentElement.hasAttribute(\"data-theme\"), \"return to system theme\");\n\n    $(\"search-input\").value = \"Pulsewire CI Story One\";\n    $(\"search-form\").requestSubmit();\n    await until(() => $(\"page-caption\").textContent === \"1 shown\" && document.querySelectorAll(\".entry-card\").length === 1, \"real search\");\n    $(\"read-filter\").value = \"unread\";\n    $(\"read-filter\").dispatchEvent(new Event(\"change\", { bubbles: true }));\n    await until(() => $(\"page-caption\").textContent === \"1 shown\", \"real unread filter\");\n    $(\"read-filter\").value = \"all\";\n    $(\"read-filter\").dispatchEvent(new Event(\"change\", { bubbles: true }));\n    await until(() => $(\"page-caption\").textContent === \"1 shown\", \"restore all filter\");\n    document.activeElement.blur();\n    document.dispatchEvent(new KeyboardEvent(\"keydown\", { key: \"/\", bubbles: true }));\n    check(document.activeElement === $(\"search-input\"), \"search shortcut\");\n\n    searchSource();\n    check($(\"source-dialog\").open && $(\"source-results\").textContent.includes(\"pulsewire-ci-real-rss\"), \"live source catalog\");\n    check(action(\"+ Subscribe\"), \"can subscribe\");\n    action(\"+ Subscribe\").click();\n    await until(() => action(\"Unsubscribe\") && $(\"subscription-count\").textContent === \"(1)\", \"real subscription create\");\n    check(action(\"☆ Favorite\"), \"can favorite feed\");\n    action(\"☆ Favorite\").click();\n    await until(() => action(\"★ Unfavorite\") && $(\"favorite-count\").textContent === \"(1)\", \"real favorite create\");\n    $(\"source-dialog\").close();\n\n    $(\"folder-button\").click();\n    check($(\"folder-dialog\").open, \"folder dialog\");\n    $(\"folder-name\").value = \"PW060 Live Browser QA\";\n    $(\"new-folder-form\").requestSubmit();\n    await until(() => $(\"folder-list\").textContent.includes(\"PW060 Live Browser QA\"), \"real folder create\");\n    $(\"folder-dialog\").close();\n    const folderButton = Array.from($(\"folder-list\").querySelectorAll(\"button\")).find((button) => button.textContent === \"PW060 Live Browser QA\");\n    check(folderButton, \"new folder appears in navigation\");\n    folderButton.click();\n    await until(() => $(\"stream-title\").textContent === \"PW060 Live Browser QA\" && $(\"page-caption\").textContent !== \"Loading…\", \"real folder view\");\n    searchSource();\n    await until(() => action(\"Add to folder\"), \"folder membership UI loaded\");\n    action(\"Add to folder\").click();\n    await until(() => action(\"Remove from folder\") && document.querySelectorAll(\".entry-card\").length >= 2, \"real folder assignment and entries\");\n    action(\"Remove from folder\").click();\n    await until(() => action(\"Add to folder\"), \"real folder feed removal\");\n    action(\"Unsubscribe\").click();\n    await until(() => action(\"+ Subscribe\") && $(\"subscription-count\").textContent === \"(0)\", \"real unsubscribe cleanup\");\n    action(\"★ Unfavorite\").click();\n    await until(() => action(\"☆ Favorite\") && $(\"favorite-count\").textContent === \"(0)\", \"real unfavorite cleanup\");\n    $(\"source-dialog\").close();\n    $(\"folder-button\").click();\n    const folderRow = Array.from($(\"folder-manager\").children).find((node) => node.textContent.includes(\"PW060 Live Browser QA\"));\n    check(folderRow, \"folder available for deletion\");\n    const remove = Array.from(folderRow.querySelectorAll(\"button\")).find((button) => button.textContent === \"Delete\");\n    check(remove, \"folder delete control\");\n    window.confirm = () => true;\n    remove.click();\n    await until(() => !$(\"folder-list\").textContent.includes(\"PW060 Live Browser QA\"), \"real folder delete cleanup\");\n    $(\"folder-dialog\").close();\n    $(\"logout-button\").click();\n    await until(() => !$(\"login-view\").hidden && $(\"reader-app\").hidden, \"real logout\");\n    check(!sessionStorage.getItem(\"pulsewire.reader.token\"), \"browser token cleared\");\n    result(\"pass\", \"live Postgres login, RSS entries, read/unread, search, theme, keyboard, subscriptions, favorite feed, folder CRUD/membership, logout\");\n  } catch (error) {\n    result(\"fail\", String(error.stack || error));\n  }\n})();"
  .replace("__PW060_USER__", JSON.stringify(username))
  .replace("__PW060_PASS__", JSON.stringify(password));

const server = createServer((req, res) => {
  const target = new URL(req.url, upstream);
  if (target.pathname === "/__pw060_live_test.js") {
    res.writeHead(200, { "content-type": "text/javascript; charset=utf-8", "cache-control": "no-store" });
    res.end(harness);
    return;
  }
  const incoming = request({
    hostname: upstream.hostname,
    port: upstream.port,
    method: req.method,
    path: target.pathname + target.search,
    headers: { ...req.headers, host: upstream.host }
  }, (remote) => {
    if (target.pathname === "/reader" && req.method === "GET") {
      const chunks = [];
      remote.on("data", (chunk) => chunks.push(chunk));
      remote.on("end", () => {
        const page = Buffer.concat(chunks).toString("utf8");
        if (!page.includes("</head>")) {
          res.writeHead(502);
          res.end("Expected real Pulsewire reader HTML");
          return;
        }
        const headers = { ...remote.headers };
        delete headers["content-length"];
        delete headers["transfer-encoding"];
        res.writeHead(remote.statusCode, headers);
        res.end(page.replace("</head>", '<script src="/__pw060_live_test.js" defer></script></head>'));
      });
    } else {
      res.writeHead(remote.statusCode, remote.headers);
      remote.pipe(res);
    }
  });
  incoming.on("error", () => {
    if (!res.headersSent) res.writeHead(502);
    res.end("Disposable Pulsewire test server unavailable");
  });
  req.pipe(incoming);
});
const profile = mkdtempSync(join(tmpdir(), "pulsewire-live-reader-"));
try {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const origin = "http://127.0.0.1:" + server.address().port + "/reader";
  for (const width of [1280, 440]) {
    const output = await new Promise((resolve, reject) => {
      const child = spawn(process.env.CHROME_BIN || "google-chrome", [
        "--headless", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
        "--disable-background-networking", "--no-first-run", "--no-default-browser-check",
        "--user-data-dir=" + join(profile, String(width)),
        "--window-size=" + width + ",850", "--virtual-time-budget=35000",
        "--dump-dom", origin
      ], { stdio: ["ignore", "pipe", "pipe"], timeout: 90000 });
      let stdout = "";
      let stderr = "";
      child.stdout.on("data", (data) => { stdout += data; });
      child.stderr.on("data", (data) => { stderr += data; });
      child.on("error", reject);
      child.on("close", (code) => {
        if (code !== 0) reject(new Error("Chromium exited " + code + ": " + stderr.slice(-1000)));
        else resolve(stdout);
      });
    });
    const verdict = output.match(/<pre id="live-smoke-result">([^<]*)<\/pre>/);
    assert.match(output, /<html[^>]*data-smoke="pass"/,
      "Live browser at " + width + "px: " + (verdict?.[1] || output.slice(-1600)));
    console.log("Real PostgreSQL Pulsewire browser smoke passed at width " + width + "px");
  }
} finally {
  server.close();
  rmSync(profile, { recursive: true, force: true });
}
