/**
 * Headless Chromium audit for Suna chat/knowledge (synthetic IPC).
 * Not equivalent to native WebView2 / standard INP.
 */
const fs = require("node:fs");
const path = require("node:path");
const { performance } = require("node:perf_hooks");
const { chromium } = require("@playwright/test");

const PAGE_SIZE = 80;
const ROUNDS = 3;
const base = process.env.SUNA_AUDIT_BASE || "http://127.0.0.1:4387";
const output = process.env.SUNA_AUDIT_OUTPUT
  || path.join(__dirname, "../../../.suna-temp/performance-audit-20261017-round3-rework");

function percentile(values, p) {
  if (!values.length) return 0;
  const sorted = values.slice().sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))];
}

function summarizeTyping(samples) {
  return {
    samples,
    sampleCount: samples.length,
    medianMs: percentile(samples, 0.5),
    p90Ms: percentile(samples, 0.9),
  };
}

async function fixture(page, { messageCount }) {
  await page.addInitScript(({ messageCount, pageSize }) => {
    const now = "2026-10-17T00:00:00Z";
    const conversationId = "perf-conversation";
    const messages = Array.from({ length: messageCount }, (_, i) => ({
      id: `message-${i}`,
      conversation_id: conversationId,
      role: i % 2 ? "assistant" : "user",
      created_at: now,
      response_json: null,
      content: i % 2
        ? `### 合成材料分析 ${i}\n\n本段仅为性能测量使用的合成数据。**热处理与强度**需要结合成分和工艺验证。\n\n- 碳含量：0.18%\n- 温度：850 °C\n- 屈服强度：355 MPa\n\n| 指标 | 值 |\n| --- | --- |\n| 强度 | 355 |\n| 温度 | 850 |\n\n公式：$E = mc^2$。\n[[ASSISTANT_MARKER_${i}]]`
        : `合成研究问题 ${i}：分析热处理工艺与材料性能的关系。[[USER_MARKER_${i}]]`,
    }));
    const kb = {
      id: "kb-1", name: "合成知识库", description: "隔离性能测量", tags: [],
      library_type: "research", visibility: "private", created_at: now, updated_at: now,
    };
    const audit = window.__perfAudit = {
      calls: {}, volumes: {}, settings: {}, marks: {}, longTasks: [], errors: [], unknown: [],
    };
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        audit.longTasks.push({ start: entry.startTime, duration: entry.duration });
      }
    }).observe({ type: "longtask", buffered: true });
    const observer = new MutationObserver(() => {
      if (!audit.marks.shell && document.querySelector(".suna-new-app")) audit.marks.shell = performance.now();
      if (!audit.marks.splashRemoved && document.querySelector(".suna-new-app") && !document.getElementById("suna-boot-screen")) {
        audit.marks.splashRemoved = performance.now();
      }
    });
    observer.observe(document, { childList: true, subtree: true });
    const callbacks = new Map();
    let callbackId = 1;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
      transformCallback(callback) {
        const id = callbackId++;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id) { callbacks.delete(id); },
      async invoke(command, args = {}) {
        audit.calls[command] = (audit.calls[command] || 0) + 1;
        const track = (payload) => {
          audit.volumes[command] = (audit.volumes[command] || 0) + JSON.stringify(payload ?? null).length;
        };
        if (command === "boot_timing") { audit.marks[args.stage] = args.elapsedMs; return null; }
        if (command === "get_setting") {
          audit.settings[args.key] = (audit.settings[args.key] || 0) + 1;
          if (args.key === "ui.theme") return JSON.stringify({ version: 1, preference: "dark" });
          if (args.key === "ui.preferences") {
            return JSON.stringify({ restoreSession: true, saveDrafts: false, autoUpdate: false, enableAnimations: false });
          }
          return null;
        }
        if (command === "list_conversations") {
          const payload = messageCount
            ? [{ id: conversationId, title: "合成长对话", created_at: now, updated_at: now, pinned: false, archived: false }]
            : [];
          track(payload);
          return payload;
        }
        if (command === "list_messages_page") {
          const request = args.request || args;
          const limit = Math.max(1, Math.min(pageSize, Number(request.limit) || pageSize));
          let endExclusive = messages.length;
          if (request.beforeRowid != null) {
            const before = Number(request.beforeRowid);
            const idx = messages.findIndex((item) => Number(item.id.replace("message-", "")) === before);
            if (idx < 0) {
              const payload = { messages: [], has_older: false, oldest_rowid: null, oldest_created_at: null };
              track(payload);
              return payload;
            }
            endExclusive = idx;
          }
          const start = Math.max(0, endExclusive - limit);
          const slice = messages.slice(start, endExclusive);
          if (slice.length > limit) throw new Error("mock violated page limit");
          const oldest = slice[0];
          const oldestIndex = oldest ? Number(oldest.id.replace("message-", "")) : null;
          const payload = {
            messages: slice,
            has_older: start > 0,
            oldest_rowid: oldestIndex,
            oldest_created_at: oldest?.created_at || null,
          };
          track(payload);
          return payload;
        }
        if (command === "get_conversation_draft") return "";
        if (command === "list_skills") return { skills: [], errors: [] };
        if (command === "list_postgres_knowledge_bases") { const payload = [kb]; track(payload); return payload; }
        if (command === "get_postgres_knowledge_base_metrics") {
          const payload = [{ knowledge_base_id: kb.id, document_count: 1, chunk_count: 500, embedded_chunk_count: 100 }];
          track(payload);
          return payload;
        }
        if (command === "get_postgres_knowledge_health") {
          const payload = {
            knowledge_base_count: 1, document_count: 1, active_document_count: 1, version_count: 1,
            chunk_count: 500, indexed_chunk_count: 100, active_task_count: 1,
          };
          track(payload);
          return payload;
        }
        if (command === "list_postgres_ingestion_jobs") {
          const payload = [{ id: "job-1", knowledge_base_id: kb.id, state: "embedding", progress: 20, attempts: 1, created_at: now, updated_at: now }];
          track(payload);
          return payload;
        }
        if (command === "list_postgres_documents" || command === "list_knowledge_documents") {
          const payload = [{
            id: "document-1", knowledge_base_id: kb.id, display_name: "合成文献.pdf", source_kind: "pdf",
            file_size: 1000, active_version_id: "version-1", created_at: now, updated_at: now,
          }];
          track(payload);
          return payload;
        }
        if (command === "list_postgres_knowledge_chunks") {
          const payload = Array.from({ length: 20 }, (_, i) => ({
            id: `chunk-${i}`, document_id: "document-1", document_name: "合成文献.pdf", version_id: "version-1",
            ordinal: i, title_path: "合成片段", text: "合成片段", source_location: {},
          }));
          track(payload);
          return payload;
        }
        if (command === "plugin:event|listen") return callbackId++;
        if (command === "plugin:window|is_maximized") return false;
        if (command.startsWith("list_") || command === "recover_agent_runs" || command === "replay_agent_run") {
          track([]);
          return [];
        }
        audit.unknown.push({ command, args });
        return null;
      },
    };
  }, { messageCount, pageSize: PAGE_SIZE });
}

async function snapshot(page) {
  return page.evaluate(() => {
    const resources = performance.getEntriesByType("resource");
    const fonts = resources.filter((entry) => /\.woff2?(?:$|\?)/.test(entry.name));
    const scripts = resources.filter((entry) => /\.js(?:$|\?)/.test(entry.name));
    return {
      marks: window.__perfAudit.marks,
      ipc: window.__perfAudit.calls,
      ipcVolumes: window.__perfAudit.volumes,
      settings: window.__perfAudit.settings,
      unknownIpc: window.__perfAudit.unknown,
      paints: performance.getEntriesByType("paint").map(({ name, startTime }) => ({ name, startTime })),
      domNodes: document.getElementsByTagName("*").length,
      loadedMessages: document.querySelectorAll("[data-message-id]").length,
      loadedUserTurns: document.querySelectorAll(".suna-chat-user-turn[data-agent-user-turn]").length,
      loadedAssistantBlocks: document.querySelectorAll(".suna-chat-assistant-turn").length,
      richTextTables: document.querySelectorAll(".suna-chat-answer table, .ai-markdown-body table").length,
      richTextStrong: document.querySelectorAll(".suna-chat-answer strong, .ai-markdown-body strong").length,
      fontRequests: fonts.length,
      fontDecodedBytes: fonts.reduce((sum, entry) => sum + (entry.decodedBodySize || 0), 0),
      jsDecodedBytes: scripts.reduce((sum, entry) => sum + (entry.decodedBodySize || 0), 0),
      jsTransferBytes: scripts.reduce((sum, entry) => sum + (entry.transferSize || 0), 0),
      jsChunkCount: scripts.length,
      cssDecodedBytes: resources.filter((entry) => /\.css(?:$|\?)/.test(entry.name)).reduce((sum, entry) => sum + (entry.decodedBodySize || 0), 0),
      longTasks: window.__perfAudit.longTasks.slice(),
      heapUsed: performance.memory?.usedJSHeapSize,
    };
  });
}

async function typeSamples(page, count = 10) {
  const input = page.locator("textarea").first();
  await input.focus();
  await page.evaluate(() => { window.__perfAudit.longTasks = []; });
  const samples = [];
  for (let key = 0; key < count; key++) {
    const before = performance.now();
    await input.press("a");
    await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    samples.push(performance.now() - before);
  }
  const after = await snapshot(page);
  return {
    typing: summarizeTyping(samples),
    longTasks: after.longTasks,
    longTasksMs: after.longTasks.reduce((sum, entry) => sum + entry.duration, 0),
    domNodes: after.domNodes,
  };
}

async function waitForFirstPageReady(page, messageCount) {
  await page.locator("textarea").first().waitFor({ timeout: 120000 });
  await page.locator("#suna-boot-screen").waitFor({ state: "detached", timeout: 120000 });
  if (!messageCount) return;
  const expectedUsers = Math.min(Math.floor(PAGE_SIZE / 2), Math.floor(messageCount / 2));
  await page.waitForFunction(
    (expected) => document.querySelectorAll(".suna-chat-user-turn[data-agent-user-turn]").length >= expected,
    expectedUsers,
    { timeout: 120000 },
  );
  const newestAssistant = messageCount - 1 - ((messageCount - 1) % 2 === 0 ? 1 : 0);
  const markerIndex = messageCount % 2 === 0 ? messageCount - 1 : messageCount - 2;
  if (markerIndex >= 0 && markerIndex % 2 === 1) {
    await page.getByText(`[[ASSISTANT_MARKER_${markerIndex}]]`, { exact: false }).waitFor({ timeout: 120000 });
    await page.waitForFunction(() => {
      const tables = document.querySelectorAll(".suna-chat-answer table, .ai-markdown-body table").length;
      const strong = document.querySelectorAll(".suna-chat-answer strong, .ai-markdown-body strong").length;
      return tables > 0 && strong > 0;
    }, null, { timeout: 120000 });
  } else if (expectedUsers > 0) {
    await page.locator("[data-message-id]").first().waitFor({ timeout: 120000 });
  }
}

async function loadAllOlderPages(page, messageCount) {
  let guard = 0;
  while (guard++ < 50) {
    const button = page.getByRole("button", { name: "加载更早消息" });
    if (!(await button.count()) || !(await button.isEnabled().catch(() => false))) break;
    await button.click();
    await page.waitForTimeout(200);
  }
  await page.waitForFunction(
    (expected) => document.querySelectorAll("[data-message-id]").length >= expected,
    messageCount,
    { timeout: 180000 },
  );
  await page.getByText("[[USER_MARKER_0]]", { exact: false }).waitFor({ timeout: 120000 });
  if (messageCount > 1) {
    await page.getByText("[[ASSISTANT_MARKER_1]]", { exact: false }).waitFor({ timeout: 120000 });
  }
  await page.waitForFunction(() => {
    return document.querySelectorAll(".suna-chat-answer table, .ai-markdown-body table").length > 0
      && document.querySelectorAll(".suna-chat-answer strong, .ai-markdown-body strong").length > 0;
  }, null, { timeout: 120000 });
}

async function measureScrollStability(page) {
  // Use the oldest currently visible message as anchor (first page of a 500-message chat).
  const before = await page.evaluate(() => {
    const el = document.querySelector("[data-message-id]");
    if (!el) return null;
    const box = el.getBoundingClientRect();
    return {
      id: el.getAttribute("data-message-id"),
      top: box.top,
      text: el.textContent?.slice(0, 80) || "",
    };
  });
  if (!before?.id) return { skipped: true, reason: "no visible message anchor" };
  await page.getByRole("button", { name: "加载更早消息" }).click();
  await page.waitForFunction((id) => {
    const nodes = document.querySelectorAll("[data-message-id]");
    return nodes.length > 80 && !!document.querySelector(`[data-message-id="${id}"]`);
  }, before.id, { timeout: 60000 });
  await page.waitForTimeout(200);
  const after = await page.evaluate((id) => {
    const el = document.querySelector(`[data-message-id="${id}"]`);
    if (!el) return null;
    const box = el.getBoundingClientRect();
    return { id, top: box.top, text: el.textContent?.slice(0, 80) || "" };
  }, before.id);
  return {
    skipped: false,
    messageId: before.id,
    beforeTop: before.top,
    afterTop: after?.top ?? null,
    deltaPx: after ? after.top - before.top : null,
    beforeSnippet: before.text,
    afterSnippet: after?.text ?? null,
  };
}

async function measureChat(browser, scenario) {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 820 },
    locale: "zh-CN",
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  const errors = [];
  const failedRequests = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("requestfailed", (request) => failedRequests.push({ url: request.url(), error: request.failure()?.errorText }));
  await page.route("**/*", (route) => (
    new URL(route.request().url()).origin === new URL(base).origin ? route.continue() : route.abort()
  ));
  await fixture(page, scenario);
  const started = performance.now();
  await page.goto(`${base}/chat`, { waitUntil: "load", timeout: 120000 });
  await waitForFirstPageReady(page, scenario.messageCount);
  const readyWallMs = performance.now() - started;
  const firstPage = await snapshot(page);

  let typingDuringLoad = null;
  let typingStable = null;
  let scrollStability = null;
  let fullLoad = null;

  if (scenario.loadAll && scenario.messageCount > PAGE_SIZE) {
    typingDuringLoad = await typeSamples(page, 10);
    await loadAllOlderPages(page, scenario.messageCount);
    fullLoad = await snapshot(page);
    typingStable = await typeSamples(page, 10);
  } else if (scenario.loadAll && scenario.messageCount > 0) {
    typingStable = await typeSamples(page, 10);
    fullLoad = firstPage;
  } else {
    typingStable = await typeSamples(page, 10);
  }

  if (scenario.scrollOlder && scenario.messageCount > PAGE_SIZE) {
    scrollStability = await measureScrollStability(page);
  }

  const asserted = await page.evaluate((expected) => ({
    messageNodes: document.querySelectorAll("[data-message-id]").length,
    expected,
    hasUniqueUser: !!document.body.innerText.includes("[[USER_MARKER_"),
    hasUniqueAssistant: expected > 1 ? document.body.innerText.includes("[[ASSISTANT_MARKER_") : true,
    tables: document.querySelectorAll(".suna-chat-answer table, .ai-markdown-body table").length,
    strong: document.querySelectorAll(".suna-chat-answer strong, .ai-markdown-body strong").length,
  }), scenario.loadAll ? scenario.messageCount : Math.min(PAGE_SIZE, scenario.messageCount));

  await context.close();
  return {
    ...scenario,
    readyWallMs,
    firstPage,
    fullLoad,
    typingDuringLoad,
    typingStable,
    scrollStability,
    asserted,
    errors,
    failedRequests,
  };
}

function summarizeResults(results) {
  const byName = {};
  for (const item of results.chat) {
    byName[item.name] ||= [];
    byName[item.name].push(item);
  }
  const summary = {};
  for (const [name, items] of Object.entries(byName)) {
    const ready = items.map((item) => item.readyWallMs);
    const typing = items.map((item) => item.typingStable?.typing.medianMs || 0);
    const typingP90 = items.map((item) => item.typingStable?.typing.p90Ms || 0);
    summary[name] = {
      rounds: items.length,
      readyWallMsMedian: percentile(ready, 0.5),
      typingMedianMsMedian: percentile(typing, 0.5),
      typingP90MsMedian: percentile(typingP90, 0.5),
      loadedMessages: items[0]?.asserted?.messageNodes,
      domNodes: items[0]?.typingStable?.domNodes ?? items[0]?.firstPage?.domNodes,
      scrollDeltaPx: items.map((item) => item.scrollStability?.deltaPx).filter((value) => value != null),
      errors: items.flatMap((item) => item.errors),
    };
  }
  return summary;
}

(async () => {
  fs.mkdirSync(output, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const results = {
    commit: process.env.SUNA_AUDIT_COMMIT || "round3-rework-uncommitted",
    environment: "Windows headless Chromium, production dist, loopback HTTP, synthetic IPC; keypress-to-double-rAF is not standard INP",
    browser: browser.version(),
    pageSize: PAGE_SIZE,
    chat: [],
    knowledge: null,
    summary: null,
  };
  try {
    const scenarios = [
      { name: "empty-first-page", messageCount: 0, loadAll: false, scrollOlder: false },
      { name: "100-first-page", messageCount: 100, loadAll: false, scrollOlder: false },
      { name: "500-first-page", messageCount: 500, loadAll: false, scrollOlder: false },
      { name: "100-full-rich", messageCount: 100, loadAll: true, scrollOlder: false },
      { name: "500-full-rich", messageCount: 500, loadAll: true, scrollOlder: false },
      { name: "500-scroll-older", messageCount: 500, loadAll: false, scrollOlder: true },
    ];
    for (let round = 0; round < ROUNDS; round++) {
      for (const scenario of scenarios) {
        const item = await measureChat(browser, { ...scenario, round });
        results.chat.push(item);
        results.summary = summarizeResults(results);
        fs.writeFileSync(path.join(output, "browser-results.json"), JSON.stringify(results, null, 2));
        process.stdout.write(`${JSON.stringify({
          round,
          scenario: scenario.name,
          readyWallMs: Math.round(item.readyWallMs),
          typingMedianMs: Math.round(item.typingStable?.typing.medianMs || 0),
          typingP90Ms: Math.round(item.typingStable?.typing.p90Ms || 0),
          loadedMessages: item.asserted.messageNodes,
          scrollDeltaPx: item.scrollStability?.deltaPx ?? null,
          errors: item.errors.length,
        })}\n`);
      }
    }
    const context = await browser.newContext({ viewport: { width: 1280, height: 820 }, locale: "zh-CN", reducedMotion: "reduce" });
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.route("**/*", (route) => (
      new URL(route.request().url()).origin === new URL(base).origin ? route.continue() : route.abort()
    ));
    await fixture(page, { messageCount: 0 });
    await page.goto(`${base}/knowledge`, { waitUntil: "load" });
    await page.getByRole("heading", { name: "知识中心", exact: true }).waitFor({ timeout: 60000 });
    const initial = await snapshot(page);
    await page.waitForTimeout(6200);
    const afterPoll = await snapshot(page);
    results.knowledge = { initial, afterPoll, errors };
    results.summary = summarizeResults(results);
    fs.writeFileSync(path.join(output, "browser-results.json"), JSON.stringify(results, null, 2));
    fs.writeFileSync(path.join(output, "browser-summary.json"), JSON.stringify(results.summary, null, 2));
    await context.close();
  } finally {
    await browser.close();
  }
})().catch((error) => {
  process.stderr.write(`${error.stack}\n`);
  process.exitCode = 1;
});
