// PW-061: compare the checked-in router's actual method/path matrix against
// the documented public API. Node standard library only; no npm or codegen.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const here = new URL("../crates/server/", import.meta.url);
const router = readFileSync(fileURLToPath(new URL("src/handlers/mod.rs", here)), "utf8");
const spec = JSON.parse(readFileSync(fileURLToPath(new URL("res/openapi.json", here)), "utf8"));
const model = readFileSync(fileURLToPath(new URL("src/models.rs", here)), "utf8");
const routes = [...router.matchAll(/\.route\("([^"]+)",\s*(get|post|delete|patch)\(/g)]
  .map((match) => [match[2], match[1].replace(/:([a-z_]+)/g, "{$1}")]);
const actual = new Set(routes.map(([method, path]) => method.toUpperCase() + " " + path));
const documented = new Set(Object.entries(spec.paths).flatMap(([path, operations]) =>
  Object.keys(operations).map((method) => method.toUpperCase() + " " + path)));
assert.deepEqual([...actual].sort(), [...documented].sort(), "OpenAPI must describe every served method and no removed method");

const authless = new Set(["/health", "/ready", "/docs", "/openapi.json", "/reader", "/reader/app.js", "/reader/styles.css", "/v1/auth/login"]);
for (const [path, operations] of Object.entries(spec.paths)) {
  for (const [method, op] of Object.entries(operations)) {
    if (authless.has(path)) {
      assert.equal(op.security, undefined, method + " " + path + " should be public");
    } else {
      assert.deepEqual(op.security, [{ bearerAuth: [] }], "Auth contract: " + method + " " + path);
      assert.ok(op.responses["401"], "Document unauthorized response: " + method + " " + path);
    }
  }
}
for (const name of ["PasswordResetRequest", "PasswordResetConfirm", "PasswordResetResponse"]) {
  assert.ok(!spec.components.schemas[name], "Removed insecure password-reset schema: " + name);
}
const list = spec.components.schemas.EntryListResponse;
for (const field of ["items", "next_offset", "next_cursor", "since"]) {
  assert.ok(list.required.includes(field), "EntryListResponse required: " + field);
  assert.ok(list.properties[field], "EntryListResponse property: " + field);
}
for (const path of ["/v1/entries", "/v1/entries/search", "/v1/folders/{folder_id}/entries", "/v1/feeds/{feed_id}/entries"]) {
  const operation = spec.paths[path].get;
  assert.ok(operation.description.includes("descending ingestion item ID"), path + " documents stable order");
  assert.ok(operation.description.includes("next_offset is null"), path + " documents pagination stop");
  for (const name of ["limit", "offset", "read"]) {
    assert.ok(operation.parameters.some((parameter) => parameter.name === name), path + " parameter " + name);
  }
}
assert.ok(spec.paths["/v1/entries/unread/count"].get.description.includes("subscribed"), "Unread count scope");
assert.ok(spec.paths["/v1/feeds/counts"].get.description.includes("full feed catalog"), "Feed count scope");
for (const [name, schema] of Object.entries(spec.components.schemas)) {
  // Refuse to describe DTO fields that are absent from their Rust definitions.
  const found = model.match(new RegExp("pub struct " + name + "\\s*\\{([\\s\\S]*?)\\n\\}"));
  if (!found) continue;
  const members = new Set([...found[1].matchAll(/\bpub\s+(\w+)\s*:/g)].map((match) => match[1]));
  for (const field of Object.keys(schema.properties || {})) {
    assert.ok(members.has(field), name + "." + field + " missing from Rust model");
  }
}
console.log("PW-061 OpenAPI, authentication, DTO and pagination contract checks passed (" + actual.size + " routes).");
