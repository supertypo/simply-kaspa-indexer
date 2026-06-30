use axum::response::Html;

pub const PATH: &str = "/admin";

pub async fn get_admin() -> Html<&'static str> {
    Html(ADMIN_HTML)
}

const ADMIN_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Simply Kaspa Indexer</title>
  <style>
    :root {
      color-scheme: light dark;
      --bg: #f7f8fb;
      --panel: #ffffff;
      --text: #17202a;
      --muted: #657281;
      --line: #d8dee8;
      --accent: #146c5f;
      --warn: #a15c00;
      --bad: #b42318;
      --ok: #0e7a45;
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --bg: #111417;
        --panel: #191e23;
        --text: #eef2f5;
        --muted: #9aa6b2;
        --line: #2d343c;
      }
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      background: var(--bg);
      color: var(--text);
      font: 14px/1.45 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
      padding: 18px 24px;
      border-bottom: 1px solid var(--line);
      background: var(--panel);
    }
    h1 { margin: 0; font-size: 20px; font-weight: 650; letter-spacing: 0; }
    main { width: min(1180px, 100%); margin: 0 auto; padding: 20px; }
    .toolbar, .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
    button, input {
      min-height: 36px;
      border: 1px solid var(--line);
      background: var(--panel);
      color: var(--text);
      border-radius: 6px;
      padding: 7px 10px;
      font: inherit;
    }
    button { cursor: pointer; }
    button.primary { background: var(--accent); border-color: var(--accent); color: #fff; }
    input { min-width: min(520px, 100%); }
    .actions input { flex: 1; min-width: min(620px, 100%); }
    .grid { display: grid; grid-template-columns: repeat(12, 1fr); gap: 14px; }
    .panel {
      grid-column: span 6;
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
      overflow: hidden;
    }
    .panel.wide { grid-column: span 12; }
    .panel h2 {
      margin: 0;
      padding: 12px 14px;
      border-bottom: 1px solid var(--line);
      font-size: 14px;
      font-weight: 650;
    }
    .content { padding: 14px; }
    .kv { display: grid; grid-template-columns: minmax(120px, 210px) 1fr; gap: 8px 12px; }
    .key { color: var(--muted); }
    .mono { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; overflow-wrap: anywhere; }
    .status { font-weight: 700; }
    .status.UP, .status.ok { color: var(--ok); }
    .status.WARN { color: var(--warn); }
    .status.DOWN, .status.bad { color: var(--bad); }
    table { width: 100%; border-collapse: collapse; }
    th, td { padding: 9px 10px; text-align: left; border-bottom: 1px solid var(--line); vertical-align: top; }
    th { color: var(--muted); font-weight: 600; }
    tr:last-child td { border-bottom: 0; }
    .empty { color: var(--muted); }
    @media (max-width: 780px) {
      header { align-items: flex-start; flex-direction: column; padding: 16px; }
      main { padding: 14px; }
      .panel { grid-column: span 12; }
      .kv { grid-template-columns: 1fr; }
      input { width: 100%; }
    }
  </style>
</head>
<body>
  <header>
    <h1>Simply Kaspa Indexer</h1>
    <div class="toolbar">
      <input id="search" placeholder="Search block, transaction, or address">
      <button class="primary" onclick="runSearch()">Search</button>
      <button onclick="refresh()">Refresh</button>
    </div>
  </header>
  <main>
    <div class="grid">
      <section class="panel">
        <h2>Status</h2>
        <div id="status" class="content empty">Loading</div>
      </section>
      <section class="panel">
        <h2>Health</h2>
        <div id="health" class="content empty">Loading</div>
      </section>
      <section class="panel wide">
        <h2>Recent Blocks</h2>
        <div id="blocks" class="content empty">Loading</div>
      </section>
      <section class="panel wide">
        <h2>Search Results</h2>
        <div id="results" class="content empty">No query</div>
      </section>
      <section class="panel wide">
        <h2>Address Lookup</h2>
        <div class="content">
          <div class="actions">
            <input id="address" placeholder="kaspa: address">
            <button class="primary" onclick="queryAddress('transactions')">Transactions</button>
            <button onclick="queryAddress('balance')">Balance</button>
            <button onclick="queryAddress('utxos')">UTXOs</button>
          </div>
        </div>
      </section>
      <section class="panel wide">
        <h2>Lookup Details</h2>
        <div id="details" class="content empty">No address query</div>
      </section>
    </div>
  </main>
  <script>
    const api = (path) => `${location.pathname.replace(/\/admin\/?$/, "")}${path}`;
    const text = (value) => value === null || value === undefined || value === "" ? "-" : value;
    const short = (value) => value && value.length > 18 ? `${value.slice(0, 10)}...${value.slice(-8)}` : text(value);
    const esc = (value) => String(text(value)).replace(/[&<>"']/g, (ch) => ({
      "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#39;"
    })[ch]);
    async function get(path, options = {}) {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), options.timeout || 7000);
      try {
        const res = await fetch(api(path), { headers: { accept: "application/json" }, signal: controller.signal });
        const allowed = options.allowStatus || [];
        if (!res.ok && !allowed.includes(res.status)) throw new Error(`${res.status} ${res.statusText}`);
        return res.json();
      } catch (error) {
        if (error.name === "AbortError") throw new Error("request timed out");
        throw error;
      } finally {
        clearTimeout(timeout);
      }
    }
    function healthSyncing(health) {
      return Boolean(health?.indexer?.details?.some((item) => String(item?.reason || "").includes("behind")));
    }
    function kv(rows) {
      return `<div class="kv">${rows.map(([k, v, cls = ""]) =>
        `<div class="key">${esc(k)}</div><div class="${cls}">${esc(v)}</div>`).join("")}</div>`;
    }
    function renderStatus(status) {
      const tip = status.chainTip || {};
      document.getElementById("status").innerHTML = kv([
        ["tip hash", short(tip.hash), "mono"],
        ["tip DAA", tip.daaScore],
        ["checkpoint", short(status.checkpointHash), "mono"],
        ["checkpoint DAA", status.checkpointDaaScore],
        ["VCP distance", status.virtualChainTipDistance],
        ["tx processor", status.transactionProcessorEnabled ? "enabled" : "disabled"],
      ]);
    }
    function renderHealth(health) {
      const syncing = healthSyncing(health);
      document.getElementById("health").innerHTML = kv([
        ["overall", health.status, `status ${health.status}`],
        ["mode", syncing ? "syncing" : "ready"],
        ["kaspad", health.kaspad?.status, `status ${health.kaspad?.status}`],
        ["synced", health.kaspad?.isSynced],
        ["network", health.kaspad?.networkId],
        ["indexer", health.indexer?.status, `status ${health.indexer?.status}`],
        ["uptime", health.indexer?.info?.uptime],
      ]);
    }
    function renderBlocks(blocks) {
      document.getElementById("blocks").innerHTML = blocks.length ? `<table>
        <thead><tr><th>Hash</th><th>Blue</th><th>DAA</th><th>Txs</th><th>Chain</th></tr></thead>
        <tbody>${blocks.map(b => `<tr>
          <td class="mono" title="${esc(b.hash)}">${esc(short(b.hash))}</td>
          <td>${esc(b.blueScore)}</td>
          <td>${esc(b.daaScore)}</td>
          <td>${esc(b.transactionCount)}</td>
          <td>${b.isChainBlock ? "yes" : "no"}</td>
        </tr>`).join("")}</tbody>
      </table>` : `<span class="empty">No blocks indexed</span>`;
    }
    function renderResults(results) {
      document.getElementById("results").innerHTML = results.length ? `<table>
        <thead><tr><th>Kind</th><th>Value</th></tr></thead>
        <tbody>${results.map(r => `<tr><td>${esc(r.kind)}</td><td class="mono">${esc(r.value)}</td></tr>`).join("")}</tbody>
      </table>` : `<span class="empty">No matches</span>`;
    }
    function renderGenericTable(items) {
      if (!items.length) return `<span class="empty">No rows</span>`;
      const keys = [...new Set(items.flatMap((item) => Object.keys(item || {})))].slice(0, 8);
      return `<table><thead><tr>${keys.map((key) => `<th>${esc(key)}</th>`).join("")}</tr></thead>
        <tbody>${items.map((item) => `<tr>${keys.map((key) => {
          const value = item?.[key];
          const display = typeof value === "object" && value !== null ? JSON.stringify(value) : value;
          return `<td class="mono">${esc(display)}</td>`;
        }).join("")}</tr>`).join("")}</tbody></table>`;
    }
    function renderDetails(title, payload) {
      const rows = Array.isArray(payload) ? payload : (payload?.transactions || payload?.utxos || null);
      document.getElementById("details").innerHTML = `<div class="key">${esc(title)}</div><br>` + (
        Array.isArray(rows) ? renderGenericTable(rows) : kv(Object.entries(payload || {}).map(([key, value]) => [
          key,
          typeof value === "object" && value !== null ? JSON.stringify(value) : value,
          typeof value === "string" && value.length > 24 ? "mono" : "",
        ]))
      );
    }
    async function refresh() {
      const [status, health, blocks] = await Promise.allSettled([
        get("/api/status"),
        get("/api/health", { allowStatus: [503] }),
        get("/api/blocks/recent?limit=12"),
      ]);
      if (status.status === "fulfilled") renderStatus(status.value);
      else document.getElementById("status").innerHTML = health.status === "fulfilled" && healthSyncing(health.value)
        ? `<span class="status WARN">Indexer syncing; status endpoint is busy (${esc(status.reason.message)})</span>`
        : `<span class="status bad">${esc(status.reason.message)}</span>`;
      if (health.status === "fulfilled") renderHealth(health.value);
      else document.getElementById("health").innerHTML = `<span class="status bad">${esc(health.reason.message)}</span>`;
      if (blocks.status === "fulfilled") renderBlocks(blocks.value);
      else document.getElementById("blocks").innerHTML = health.status === "fulfilled" && healthSyncing(health.value)
        ? `<span class="status WARN">Indexer syncing; recent blocks endpoint is busy (${esc(blocks.reason.message)})</span>`
        : `<span class="status bad">${esc(blocks.reason.message)}</span>`;
    }
    async function runSearch() {
      const q = document.getElementById("search").value.trim();
      if (!q) return;
      try { renderResults(await get(`/api/search?q=${encodeURIComponent(q)}`)); }
      catch (error) { document.getElementById("results").innerHTML = `<span class="status bad">${esc(error.message)}</span>`; }
    }
    async function queryAddress(kind) {
      const address = document.getElementById("address").value.trim();
      if (!address) return;
      const path = kind === "transactions"
        ? `/api/addresses/${encodeURIComponent(address)}/transactions?limit=25`
        : kind === "balance"
          ? `/api/addresses/${encodeURIComponent(address)}/balance`
          : `/api/addresses/${encodeURIComponent(address)}/utxos?limit=25`;
      try { renderDetails(`${kind}: ${address}`, await get(path, { timeout: 12000 })); }
      catch (error) { document.getElementById("details").innerHTML = `<span class="status bad">${esc(error.message)}</span>`; }
    }
    document.getElementById("search").addEventListener("keydown", (event) => {
      if (event.key === "Enter") runSearch();
    });
    document.getElementById("address").addEventListener("keydown", (event) => {
      if (event.key === "Enter") queryAddress("transactions");
    });
    refresh();
    setInterval(refresh, 15000);
  </script>
</body>
</html>"#;
