"use strict";

// The reader ships as static assets embedded in pulsewire-server: no build chain.
// All feed/user-controlled strings are inserted with textContent, never innerHTML.
const $ = (id) => document.getElementById(id);
const state = {
  token: null,
  feeds: [],
  subscriptions: [],
  favorites: [],
  folders: [],
  folderFeeds: [],
  view: { kind: "all", id: null },
  readFilter: "all",
  search: "",
  entries: [],
  selected: null,
  pageSize: 50,
  nextOffset: 0,
  hasMore: false,
  generation: 0,
  selectionGeneration: 0,
  loading: false
};
const textNode = (tag, value, className) => {
  const node = document.createElement(tag);
  node.textContent = value == null ? "" : String(value);
  if (className) node.className = className;
  return node;
};
const button = (label, onClick, className) => {
  const node = textNode("button", label, className || "");
  node.type = "button";
  node.addEventListener("click", onClick);
  return node;
};
const safeLink = (input) => {
  if (typeof input !== "string") return null;
  try {
    const url = new URL(input);
    return url.protocol === "https:" || url.protocol === "http:" ? url.href : null;
  } catch {
    return null;
  }
};
const dateLabel = (ms) => {
  if (ms == null) return "Date unknown";
  const date = new Date(Number(ms));
  return Number.isNaN(date.getTime()) ? "Date unknown" : date.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
};
const feed = (id) => state.feeds.find((item) => item.id === id);
const feedLabel = (id) => {
  const item = feed(id);
  return item ? (item.domain || item.id) + (item.id === item.domain ? "" : " · " + item.id) : id;
};
const favoriteIds = () => new Set(state.favorites.map((item) => item.id));
const subscriptionIds = () => new Set(state.subscriptions.map((item) => item.feed_id));

function notify(message) {
  const notice = $("notice");
  notice.hidden = !message;
  notice.textContent = message || "";
}
function showLogin(message) {
  state.token = null;
  state.generation++;
  state.selectionGeneration++;
  try { sessionStorage.removeItem("pulsewire.reader.token"); } catch {}
  $("reader-app").hidden = true;
  $("login-view").hidden = false;
  $("login-error").hidden = !message;
  $("login-error").textContent = message || "";
  $("password").value = "";
  $("username").focus();
}
function showApp() {
  $("login-view").hidden = true;
  $("reader-app").hidden = false;
}
async function api(path, options = {}) {
  const headers = { Accept: "application/json" };
  if (options.body !== undefined) headers["Content-Type"] = "application/json";
  if (state.token) headers.Authorization = "Bearer " + state.token;
  let response;
  try {
    response = await fetch(path, {
      method: options.method || "GET",
      headers,
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
      cache: "no-store",
      credentials: "same-origin"
    });
  } catch {
    throw new Error("Cannot reach the Pulsewire server. Check your connection and retry.");
  }
  if (response.status === 401 && path !== "/v1/auth/login") {
    showLogin("Your session has expired. Sign in again.");
    throw new Error("Session expired");
  }
  if (!response.ok) {
    let message = "Request failed (HTTP " + response.status + ").";
    try {
      const payload = await response.json();
      message = payload?.error?.message || message;
    } catch {}
    if (response.status === 429) message += " Try again later.";
    throw new Error(message);
  }
  if (response.status === 204 || response.status === 205) return null;
  if (!(response.headers.get("content-type") || "").includes("application/json")) return null;
  return response.json();
}
function report(error) {
  if (state.token) notify(error instanceof Error ? error.message : "Unexpected error");
}
function theme(value) {
  const chosen = ["system", "light", "dark"].includes(value) ? value : "system";
  if (chosen === "system") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.setAttribute("data-theme", chosen);
  $("theme-select").value = chosen;
  try { localStorage.setItem("pulsewire.reader.theme", chosen); } catch {}
}
async function allFavorites() {
  const results = [];
  for (let offset = 0; ; offset += 200) {
    const page = await api("/v1/favorites?limit=200&offset=" + offset);
    results.push(...page);
    if (page.length < 200) return results;
  }
}
async function refreshMetadata() {
  const [feeds, subscriptions, favorites, folders] = await Promise.all([
    api("/v1/feeds"),
    api("/v1/subscriptions"),
    allFavorites(),
    api("/v1/folders")
  ]);
  state.feeds = feeds;
  state.subscriptions = subscriptions;
  state.favorites = favorites;
  state.folders = folders;
  renderNavigation();
}
async function startApp() {
  showApp();
  notify("");
  try {
    await refreshMetadata();
    await selectView({ kind: "all", id: null });
  } catch (error) {
    report(error);
  }
}
function closeSidebar() {
  $("sidebar").classList.remove("open");
  $("sidebar-shade").hidden = true;
  $("menu-button").setAttribute("aria-expanded", "false");
}
function navButton(label, view) {
  const selected = state.view.kind === view.kind && state.view.id === view.id;
  const node = button(label, () => { selectView(view).catch(report); closeSidebar(); }, "nav-item" + (selected ? " active" : ""));
  node.title = label;
  if (selected) node.setAttribute("aria-current", "page");
  return node;
}
function renderNavigation() {
  const subscriptions = $("subscription-list");
  const favorites = $("favorite-list");
  const folders = $("folder-list");
  subscriptions.replaceChildren();
  favorites.replaceChildren();
  folders.replaceChildren();
  $("all-button").classList.toggle("active", state.view.kind === "all");
  $("subscription-count").textContent = "(" + state.subscriptions.length + ")";
  $("favorite-count").textContent = "(" + state.favorites.length + ")";
  state.subscriptions.forEach((item) => subscriptions.append(navButton(feedLabel(item.feed_id), { kind: "feed", id: item.feed_id })));
  state.favorites.forEach((item) => favorites.append(navButton(feedLabel(item.id), { kind: "feed", id: item.id })));
  state.folders.forEach((item) => folders.append(navButton(item.name, { kind: "folder", id: item.id })));
}
function viewLabel() {
  if (state.view.kind === "feed") return feedLabel(state.view.id);
  if (state.view.kind === "folder") return state.folders.find((item) => item.id === state.view.id)?.name || "Folder";
  return "All ingested entries";
}
function clearDetail() {
  state.selected = null;
  state.selectionGeneration++;
  $("detail-feed").textContent = "";
  $("read-toggle").hidden = true;
  $("detail-body").replaceChildren();
  const blank = textNode("div", "", "empty-detail");
  blank.append(textNode("span", "◌", "empty-icon"), textNode("h2", "Choose an article"), textNode("p", "Select a story to read the saved summary and open its original source."));
  $("detail-body").append(blank);
  $("workspace")?.classList.remove("detail-open");
  document.querySelector(".workspace").classList.remove("detail-open");
}
async function selectView(view) {
  state.view = view;
  state.search = "";
  $("search-input").value = "";
  $("stream-title").textContent = viewLabel();
  renderNavigation();
  clearDetail();
  state.folderFeeds = [];
  if (view.kind === "folder") {
    const rows = await api("/v1/folders/" + encodeURIComponent(view.id) + "/feeds");
    if (state.view.kind !== "folder" || state.view.id !== view.id) return;
    state.folderFeeds = rows.map((item) => item.feed_id);
  }
  await loadEntries(true);
}
function endpoint(offset) {
  const q = new URLSearchParams({ limit: String(state.pageSize), offset: String(offset), read: state.readFilter });
  if (state.search) {
    q.set("q", state.search);
    if (state.view.kind === "feed") q.set("feed_id", state.view.id);
    return "/v1/entries/search?" + q.toString();
  }
  if (state.view.kind === "folder") return "/v1/folders/" + encodeURIComponent(state.view.id) + "/entries?" + q.toString();
  if (state.view.kind === "feed") return "/v1/feeds/" + encodeURIComponent(state.view.id) + "/entries?" + q.toString();
  return "/v1/entries?" + q.toString();
}
async function loadEntries(reset) {
  if (state.loading && !reset) return;
  const generation = reset ? ++state.generation : state.generation;
  const offset = reset ? 0 : state.nextOffset;
  state.loading = true;
  $("load-more").disabled = true;
  $("page-caption").textContent = "Loading…";
  if (reset) {
    state.entries = [];
    state.nextOffset = 0;
    state.hasMore = false;
    $("entry-list").replaceChildren();
  }
  try {
    const payload = await api(endpoint(offset));
    if (generation !== state.generation || !state.token) return;
    state.entries.push(...payload.items);
    state.nextOffset = payload.next_offset ?? (offset + payload.items.length);
    state.hasMore = payload.items.length === state.pageSize;
    renderEntries();
  } catch (error) {
    if (generation === state.generation) {
      if (!state.entries.length) $("entry-list").replaceChildren(textNode("p", "Unable to load entries.", "empty-list"));
      report(error);
    }
  } finally {
    if (generation === state.generation) {
      state.loading = false;
      $("load-more").disabled = false;
      $("load-more").hidden = !state.hasMore;
      $("page-caption").textContent = state.entries.length + " shown";
    }
  }
}
function renderEntries() {
  const list = $("entry-list");
  list.replaceChildren();
  for (const item of state.entries) {
    const card = button("", () => { openEntry(item.id).catch(report); }, "entry-card" + (item.is_read ? " read" : "") + (state.selected?.id === item.id ? " selected" : ""));
    const meta = textNode("div", "", "entry-meta");
    meta.append(textNode("span", feed(item.feed_id)?.domain || item.feed_id), textNode("time", dateLabel(item.published_at_ms)));
    card.append(meta, textNode("strong", item.title || "(Untitled entry)"));
    card.dataset.id = String(item.id);
    list.append(card);
  }
  if (!state.entries.length) list.append(textNode("p", "No entries match this view.", "empty-list"));
  $("load-more").hidden = !state.hasMore;
}
function showDetail(item) {
  const body = $("detail-body");
  body.replaceChildren();
  $("detail-feed").textContent = feedLabel(item.feed_id);
  const title = textNode("h2", item.title || "(Untitled entry)");
  title.id = "detail-title";
  const date = textNode("p", dateLabel(item.published_at_ms), "muted");
  body.append(title, date);
  const url = safeLink(item.link);
  if (url) {
    const a = textNode("a", "Read original article ↗", "article-link");
    a.href = url;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
    a.referrerPolicy = "no-referrer";
    body.append(a);
  }
  const summary = item.summary || item.description || "No article summary is available. Open the original article to read more.";
  body.append(textNode("p", summary, "article-copy"));
  $("read-toggle").hidden = false;
  $("read-toggle").textContent = item.is_read ? "Mark unread" : "Mark read";
  document.querySelector(".workspace").classList.add("detail-open");
}
async function openEntry(id) {
  const selectionGeneration = ++state.selectionGeneration;
  try {
    const detail = await api("/v1/entries/" + encodeURIComponent(id));
    if (selectionGeneration !== state.selectionGeneration || !state.token) return;
    state.selected = detail;
    showDetail(detail);
    renderEntries();
    if (!detail.is_read) {
      await api("/v1/entries/" + encodeURIComponent(id) + "/read", { method: "POST" });
      if (selectionGeneration !== state.selectionGeneration || !state.token) return;
      detail.is_read = true;
      const item = state.entries.find((row) => row.id === id);
      if (item) item.is_read = true;
      $("read-toggle").textContent = "Mark unread";
      renderEntries();
    }
  } catch (error) { report(error); }
}
async function toggleRead() {
  const item = state.selected;
  if (!item) return;
  try {
    await api("/v1/entries/" + encodeURIComponent(item.id) + "/read", { method: item.is_read ? "DELETE" : "POST" });
    item.is_read = !item.is_read;
    const row = state.entries.find((entry) => entry.id === item.id);
    if (row) row.is_read = item.is_read;
    $("read-toggle").textContent = item.is_read ? "Mark unread" : "Mark read";
    renderEntries();
  } catch (error) { report(error); }
}
async function mutate(path, options) {
  notify("");
  try { await api(path, options); return true; }
  catch (error) { report(error); return false; }
}
function sourceRow(item) {
  const row = textNode("div", "", "source-row");
  row.append(textNode("strong", item.domain || item.id), textNode("small", item.id + " · " + item.category));
  const actions = textNode("div", "", "source-actions");
  const subscribed = subscriptionIds().has(item.id);
  const starred = favoriteIds().has(item.id);
  actions.append(button(subscribed ? "Unsubscribe" : "+ Subscribe", async () => {
    const ok = await mutate(subscribed ? "/v1/subscriptions/" + encodeURIComponent(item.id) : "/v1/subscriptions", subscribed ? { method: "DELETE" } : { method: "POST", body: { feed_id: item.id } });
    if (!ok) return;
    if (subscribed) state.subscriptions = state.subscriptions.filter((entry) => entry.feed_id !== item.id);
    else state.subscriptions.push({ feed_id: item.id });
    renderNavigation(); renderSourceResults();
  }));
  actions.append(button(starred ? "★ Unfavorite" : "☆ Favorite", async () => {
    const ok = await mutate(starred ? "/v1/favorites/" + encodeURIComponent(item.id) : "/v1/favorites", starred ? { method: "DELETE" } : { method: "POST", body: { feed_id: item.id } });
    if (!ok) return;
    if (starred) state.favorites = state.favorites.filter((entry) => entry.id !== item.id);
    else state.favorites.push(item);
    renderNavigation(); renderSourceResults();
  }));
  if (state.view.kind === "folder") {
    const member = state.folderFeeds.includes(item.id);
    actions.append(button(member ? "Remove from folder" : "Add to folder", async () => {
      const path = "/v1/folders/" + encodeURIComponent(state.view.id) + "/feeds" + (member ? "/" + encodeURIComponent(item.id) : "");
      const ok = await mutate(path, member ? { method: "DELETE" } : { method: "POST", body: { feed_id: item.id } });
      if (!ok) return;
      if (member) state.folderFeeds = state.folderFeeds.filter((id) => id !== item.id);
      else state.folderFeeds.push(item.id);
      renderSourceResults();
      loadEntries(true).catch(report);
    }));
  }
  row.append(actions);
  return row;
}
function renderSourceResults() {
  const needle = $("source-search").value.trim().toLowerCase();
  const results = state.feeds.filter((item) => (item.id + " " + item.domain + " " + item.category + " " + item.url).toLowerCase().includes(needle)).slice(0, 40);
  $("source-results").replaceChildren(...results.map(sourceRow));
  if (!results.length) $("source-results").append(textNode("p", "No matching catalog feeds.", "muted"));
}
function openSources() {
  $("source-search").value = "";
  renderSourceResults();
  $("source-dialog").showModal();
  $("source-search").focus();
}
function folderRow(folder) {
  const row = textNode("div", "", "folder-row");
  row.append(textNode("strong", folder.name));
  const actions = textNode("div", "", "folder-actions");
  actions.append(button("Open", () => {
    $("folder-dialog").close();
    selectView({ kind: "folder", id: folder.id }).catch(report);
  }));
  actions.append(button("Rename", async () => {
    const name = window.prompt("New folder name", folder.name)?.trim();
    if (!name || name === folder.name) return;
    if (!await mutate("/v1/folders/" + folder.id, { method: "PATCH", body: { name } })) return;
    folder.name = name;
    $("stream-title").textContent = viewLabel();
    renderNavigation(); renderFolderManager();
  }));
  actions.append(button("Delete", async () => {
    if (!window.confirm('Delete folder "' + folder.name + '"? Its feeds and entries will not be deleted.')) return;
    if (!await mutate("/v1/folders/" + folder.id, { method: "DELETE" })) return;
    state.folders = state.folders.filter((item) => item.id !== folder.id);
    renderNavigation(); renderFolderManager();
    if (state.view.kind === "folder" && state.view.id === folder.id) selectView({ kind: "all", id: null }).catch(report);
  }));
  row.append(actions);
  return row;
}
function renderFolderManager() {
  $("folder-manager").replaceChildren(...state.folders.map(folderRow));
  if (!state.folders.length) $("folder-manager").append(textNode("p", "No folders yet.", "muted"));
}
function registerEvents() {
  $("login-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    $("login-button").disabled = true;
    $("login-error").hidden = true;
    try {
      const result = await api("/v1/auth/login", { method: "POST", body: { username: $("username").value.trim(), password: $("password").value } });
      state.token = result.token;
      try { sessionStorage.setItem("pulsewire.reader.token", state.token); } catch {}
      $("password").value = "";
      await startApp();
    } catch (error) {
      if (!state.token) {
        $("login-error").textContent = error.message;
        $("login-error").hidden = false;
      } else report(error);
    } finally { $("login-button").disabled = false; }
  });
  $("logout-button").addEventListener("click", async () => {
    if (state.token) {
      try { await api("/v1/auth/logout", { method: "POST" }); } catch { /* Local session is cleared regardless. */ }
    }
    showLogin("");
  });
  $("theme-select").addEventListener("change", (event) => theme(event.target.value));
  $("all-button").addEventListener("click", () => { selectView({ kind: "all", id: null }).catch(report); closeSidebar(); });
  $("sources-button").addEventListener("click", openSources);
  $("folder-button").addEventListener("click", () => { renderFolderManager(); $("folder-dialog").showModal(); });
  $("source-search").addEventListener("input", renderSourceResults);
  document.querySelectorAll(".close-dialog").forEach((node) => node.addEventListener("click", () => node.closest("dialog").close()));
  $("new-folder-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const name = $("folder-name").value.trim();
    if (!name) return;
    try {
      const folder = await api("/v1/folders", { method: "POST", body: { name } });
      state.folders.push(folder);
      $("folder-name").value = "";
      renderNavigation(); renderFolderManager();
    } catch (error) { report(error); }
  });
  $("search-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    state.search = $("search-input").value.trim();
    if (state.search && state.view.kind === "folder") {
      state.view = { kind: "all", id: null };
      $("stream-title").textContent = "Search all ingested entries";
      renderNavigation();
      notify("Search is across all ingested entries, not restricted to the folder.");
    }
    clearDetail();
    await loadEntries(true);
  });
  $("read-filter").addEventListener("change", (event) => {
    state.readFilter = event.target.value;
    clearDetail();
    loadEntries(true).catch(report);
  });
  $("load-more").addEventListener("click", () => loadEntries(false).catch(report));
  $("refresh-button").addEventListener("click", async () => {
    try { await refreshMetadata(); clearDetail(); await loadEntries(true); }
    catch (error) { report(error); }
  });
  $("read-toggle").addEventListener("click", toggleRead);
  $("back-button").addEventListener("click", () => document.querySelector(".workspace").classList.remove("detail-open"));
  $("menu-button").addEventListener("click", () => {
    const open = $("sidebar").classList.toggle("open");
    $("sidebar-shade").hidden = !open;
    $("menu-button").setAttribute("aria-expanded", String(open));
  });
  $("sidebar-shade").addEventListener("click", closeSidebar);
  document.addEventListener("keydown", (event) => {
    if ($("reader-app").hidden) return;
    if (event.key === "Escape") { closeSidebar(); document.querySelector(".workspace").classList.remove("detail-open"); return; }
    if (document.querySelector("dialog[open]")) return;
    if (event.target.closest("input, textarea, select, [contenteditable]")) return;
    if (event.key === "/") { event.preventDefault(); $("search-input").focus(); }
    if (event.key === "?") window.alert("Shortcuts: / search · j next · k previous · r toggle read · Escape back");
    if (event.key === "r") toggleRead();
    if (event.key === "j" || event.key === "k") {
      const current = state.entries.findIndex((entry) => entry.id === state.selected?.id);
      const next = event.key === "j" ? Math.min(current + 1, state.entries.length - 1) : Math.max(current - 1, 0);
      if (state.entries[next]) openEntry(state.entries[next].id).catch(report);
    }
  });
}
registerEvents();
let initialTheme = "system";
try { initialTheme = localStorage.getItem("pulsewire.reader.theme") || "system"; } catch {}
theme(initialTheme);
try { state.token = sessionStorage.getItem("pulsewire.reader.token"); } catch {}
if (state.token) startApp();
else showLogin("");
