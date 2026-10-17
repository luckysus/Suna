import { expect, test } from "@playwright/test";

// Stable IPC fixtures isolate visual checks from user data and network latency.
const pages = ["chat", "knowledge", "literature", "data", "prediction", "optimization", "experiment", "agents", "tools", "mcp", "skills", "models", "reports", "settings", "account", "about", "diagnostics"];
test.use({ viewport: { width: 1280, height: 820 }, deviceScaleFactor: 1.25 });

async function prepare(page: import("@playwright/test").Page, theme: string) {
  await page.clock.setFixedTime(new Date("2026-10-12T00:00:00Z"));
  await page.addInitScript(({ theme }) => {
    const now = "2026-10-10T00:00:00Z";
    const agent = {
      id: "knowledge", name: "Knowledge Agent", description: "知识检索和证据引用", enabled: true, status: "available", preset: true,
      systemPrompt: "只使用可追溯的知识证据，注明文献和来源。", providerId: null, toolIds: ["steel.knowledge_search"],
      permissionRestrictions: { allowFileAccess: true, allowShell: false, allowNetwork: true, allowDatabase: true, allowMcp: true, confirmDangerous: true },
      limits: { maxTurns: 20, maxToolCalls: 64, contextBudget: 32768, retries: 2, recoveryRetries: 2, runTimeoutSeconds: 1800 },
    };
    const base = { id: "kb-1", name: "钢铁材料研究", description: "材料、工艺与性能资料", library_type: "research", tags: ["钢铁"], visibility: "private", embedding_model: "BAAI/bge-m3", chunk_strategy: "semantic", created_at: now, updated_at: now };
    const tools = [{ id: "steel.knowledge_search", name: "知识检索", description: "检索文献并返回可追溯证据", source: "builtin", enabled: true }];
    const health = { knowledge_base_count: 1, document_count: 1, active_document_count: 1, version_count: 1, chunk_count: 38, indexed_chunk_count: 38, active_task_count: 0 };
    const callbacks = new Map<number, (value: unknown) => void>();
    let callbackId = 1;
    Object.assign(window, {
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} },
      __TAURI_INTERNALS__: {
        metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
        transformCallback: (callback: (value: unknown) => void) => { const id = callbackId++; callbacks.set(id, callback); return id; },
        unregisterCallback: (id: number) => callbacks.delete(id),
        invoke: async (command: string, args?: Record<string, unknown>) => {
          if (command === "get_setting") {
            if (args?.key === "ui.theme") return JSON.stringify({ version: 1, preference: theme });
            if (args?.key === "ui.preferences") return JSON.stringify({ animationEnabled: false, autoUpdate: false });
            return null;
          }
          if (command === "list_agent_profiles" || command === "list_agent_profile_presets") return [agent];
          if (command === "list_tool_capabilities") return tools;
          if (command === "list_skills") return { skills: [{ name: "paper-summary", description: "提取文献方法、结论与引用", enabled: true, source: "bundled", tags: [], path: "C:/fixtures/skills/paper-summary/SKILL.md" }], warnings: [] };
          if (command === "list_postgres_knowledge_bases") return [base];
          if (command === "list_postgres_documents") return [{ id: "doc-1", knowledge_base_id: base.id, display_name: "Q355B 热处理研究.pdf", source_kind: "pdf", file_size: 245760, active_version_id: "version-1", created_at: now, updated_at: now }];
          if (command === "get_postgres_knowledge_base_metrics") return [{ knowledge_base_id: base.id, document_count: 1, chunk_count: 38, indexed_document_count: 1, embedded_chunk_count: 38 }];
          if (command === "get_postgres_knowledge_health") return health;
          if (command === "get_knowledge_database_health") return { configured: false, config: null, connected: false, vector_extension: false, migration_version: null, message: "尚未配置 PostgreSQL 知识库" };
          if (command === "get_storage_paths") return { app_data: "C:/Suna", documents: "C:/Suna/documents", local_data: "C:/Suna/LocalData", sqlite_database: "C:/Suna/suna.sqlite3", knowledge_content: "C:/Suna/objects", models: "C:/Suna", cache: "C:/Suna/cache", logs: "C:/Suna/logs", temp: "C:/Suna/temp" };
          if (command === "get_storage_health") return { database_ok: true, current_migration_version: 35, latest_migration_version: 35, database_size_bytes: 2048, reclaimable_bytes: 512, available_disk_bytes: 50 * 1024 ** 3 };
          if (command === "get_data_directory_change") return { pending_directory: null, message: null, environment_override: false };
          if (command === "get_index_health") return { state: "healthy", reason: null, serving_mode: "hnsw", chunk_count: 38, required_rebuild_bytes: 1024, available_disk_bytes: 50 * 1024 ** 3, stale_temporary_count: 0, rebuild_task_id: null };
          if (command.startsWith("list_") || command === "recover_agent_runs" || command === "search_literature") return [];
          if (command === "plugin:event|listen") return callbackId++;
          return null;
        },
      },
    });
  }, { theme });
}

async function ready(page: import("@playwright/test").Page, section: string, theme: string) {
  await page.goto(`/${section}`);
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
  await expect(page.locator(".suna-new-content")).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  await expect(page.locator(".suna-spin:visible")).toHaveCount(0);
  await expect(page.getByText("正在加载页面")).toHaveCount(0, { timeout: 15_000 });
}

for (const theme of ["light", "dark"]) {
  for (const section of pages) {
    test(`${theme} ${section}`, async ({ page }) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await prepare(page, theme);
      await ready(page, section, theme);
      const findings = await page.evaluate(() => {
        const visible = (el: Element) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0 && r.top < innerHeight && r.bottom > 0; };
        const rgb = (value: string) => value.match(/[\d.]+/g)!.map(Number);
        const luminance = (color: number[]) => color.slice(0, 3).map((c) => c / 255).map((c) => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4).reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i], 0);
        const surface = (el: Element): number[] => {
          const c = rgb(getComputedStyle(el).backgroundColor);
          const parent = el.parentElement ? surface(el.parentElement) : [255, 255, 255];
          return c.length === 4 ? c.slice(0, 3).map((v, i) => v * c[3] + parent[i] * (1 - c[3])) : c;
        };
        const lowContrast = [...document.querySelectorAll(".suna-new-content h1,.suna-new-content h2,.suna-new-content h3,.suna-welcome-card strong,.suna-new-breadcrumb strong,.suna-new-create,.suna-new-nav-item.is-active,.suna-action-primary:not(:disabled)")].filter(visible).flatMap((el) => {
          const style = getComputedStyle(el), fg = luminance(rgb(style.color)), bg = luminance(surface(el));
          const ratio = (Math.max(fg, bg) + 0.05) / (Math.min(fg, bg) + 0.05);
          const large = parseFloat(style.fontSize) >= 24 || (parseFloat(style.fontSize) >= 18.66 && parseInt(style.fontWeight) >= 700);
          return ratio < (large ? 3 : 4.5) ? [{ text: el.textContent, ratio }] : [];
        });
        const whitePanels = document.documentElement.dataset.theme === "dark" ? [...document.querySelectorAll(".suna-welcome-card,.suna-management-card,.suna-agent-editor,.suna-agents-create,.suna-data-list,.suna-data-analysis,.suna-knowledge-main,.suna-prediction-config,.suna-prediction-result,.suna-optimization-config,.suna-optimization-result,.suna-reports-config,.suna-reports-preview")].filter(visible).filter((el) => surface(el).every((channel) => channel > 220)).map((el) => el.className) : [];
        return { lowContrast, whitePanels, overflow: document.documentElement.scrollWidth > innerWidth + 1 };
      });
      expect(errors).toEqual([]);
      expect(findings).toEqual({ lowContrast: [], whitePanels: [], overflow: false });
      if (section === "chat") {
        const box = await page.locator(".suna-chat-composer").boundingBox();
        expect(box).not.toBeNull();
        expect(box!.y + box!.height).toBeLessThanOrEqual(820);
      }
      await expect(page).toHaveScreenshot(`${theme}-${section}.png`, { animations: "disabled", caret: "hide", threshold: 0.15, maxDiffPixelRatio: 0.0001 });
    });
  }
  for (const slot of ["embedding", "reranker"]) {
    test(`${theme} ${slot} provider editor`, async ({ page }) => {
      await prepare(page, theme);
      await ready(page, "models", theme);
      const selector = page.locator(`#settings-${slot}-kind`);
      await selector.scrollIntoViewIfNeeded();
      await selector.click();
      await page.getByRole("option", { name: "阿里百炼 Qwen", exact: true }).click();
      const form = page.locator("form").filter({ has: selector });
      await expect(form.locator(`#settings-${slot}-model`)).toHaveValue(slot === "embedding" ? "qwen3.7-text-embedding" : "qwen3.7-text-rerank");
      await expect(form).toHaveScreenshot(`${theme}-${slot}-editor.png`, { animations: "disabled", caret: "hide", threshold: 0.15, maxDiffPixelRatio: 0.0001 });
    });
  }
}

test("pixel comparison detects a changed panel background", async ({ page }, testInfo) => {
  test.skip(testInfo.config.updateSnapshots !== "none", "Run the negative check only against reviewed baselines.");
  await prepare(page, "dark");
  await ready(page, "chat", "dark");
  await page.addStyleTag({ content: ".suna-welcome-card { background: #ff0000 !important; }" });
  let failure: Error | undefined;
  try {
    await expect(page).toHaveScreenshot("dark-chat.png", { timeout: 2000, threshold: 0.15, maxDiffPixelRatio: 0.0001 });
  } catch (error) { failure = error as Error; }
  expect(failure?.message).toContain("pixels");
});
