// Uses only an isolated test data directory. Keeps both original and migrated fixtures.
// node scripts/desktop-storage-smoke.mjs --root <isolated-directory> [--cdp <endpoint>]
import assert from "node:assert/strict";
import { mkdir, readFile, writeFile, access } from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
assert(option("--root"), "--root must name the isolated test directory");
const root = path.resolve(option("--root"));
const endpoint = option("--cdp", "http://127.0.0.1:9223");
let browser, page;
async function connect() {
  for (let attempt = 0; attempt < 60; attempt++) {
    try {
      browser = await chromium.connectOverCDP(endpoint, { timeout: 1000 });
      page = browser.contexts().flatMap((item) => item.pages()).find((item) => item.url().includes("127.0.0.1:1420"));
      if (!page) throw new Error("no Tauri page");
      await page.waitForFunction(() => !!window.__TAURI_INTERNALS__, { timeout: 2000 });
      await page.waitForTimeout(500);
      if ((await invoke("get_storage_health")).database_ok) return;
    } catch { await browser?.close().catch(() => {}); }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("Tauri did not restart with a healthy database");
}
const invoke = (command, args) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
const samePath = (a, b) => path.resolve(a).toLowerCase() === path.resolve(b).toLowerCase();
async function storagePanel() {
  await page.evaluate(() => { history.pushState(null, "", "/settings"); dispatchEvent(new PopStateEvent("popstate")); });
  await page.locator("#settings-tab-databases").click();
  await page.getByRole("button", { name: "刷新存储状态" }).click();
  await page.waitForTimeout(300);
}
async function restart(usingUi = false) {
  if (usingUi) await page.getByRole("button", { name: "重启并迁移" }).click().catch(() => {});
  else await invoke("restart_for_data_directory").catch(() => {});
  await browser.close().catch(() => {});
  await new Promise((resolve) => setTimeout(resolve, 1000));
  await connect();
}
await connect();
try {
  const source = (await invoke("get_storage_paths")).app_data;
  assert(path.relative(root, source) && !path.relative(root, source).startsWith(".."), "refusing to migrate a directory outside the test root");
  assert(!(await invoke("get_data_directory_change")).environment_override, "unset SUNA_DATA_DIR; use isolated SUNA_STORAGE_CONFIG_FILE instead");
  const target = path.join(root, `migrated-${Date.now()}`);
  await mkdir(path.join(source, "objects", "smoke"), { recursive: true });
  const file = path.join(source, "objects", "smoke", "中文 文献.txt");
  await writeFile(file, "Suna migration preserves actual content / 中文文献");
  const conversation = await invoke("create_conversation", { title: "真实迁移验证" });
  await invoke("append_message", { conversationId: conversation.id, role: "user", content: "迁移后历史记录必须保留", responseJson: null });
  await invoke("set_setting", { key: "validation.owned_path", valueJson: JSON.stringify({ path: file, external: path.join(root, "paper.txt") }) });
  await invoke("change_data_directory", { directory: target });
  assert(samePath((await invoke("get_storage_paths")).app_data, source), "pending migration must not change the running root");
  await storagePanel();
  await page.getByRole("button", { name: "重启并迁移" }).waitFor();
  await page.screenshot({ path: path.join(root, "migration-pending.png") });
  await restart(true);
  assert(samePath((await invoke("get_storage_paths")).app_data, target));
  assert.equal(await readFile(path.join(target, "objects", "smoke", "中文 文献.txt"), "utf8"), await readFile(file, "utf8"));
  await access(path.join(source, "suna.sqlite3"));
  const messages = await invoke("list_messages", { conversationId: conversation.id });
  assert(messages.some((message) => message.content === "迁移后历史记录必须保留"));
  const setting = JSON.parse(await invoke("get_setting", { key: "validation.owned_path" }));
  assert(samePath(setting.path, path.join(target, "objects", "smoke", "中文 文献.txt")));
  assert(samePath(setting.external, path.join(root, "paper.txt")));
  assert.equal((await invoke("get_data_directory_change")).pending_directory, null);
  await storagePanel();
  await page.screenshot({ path: path.join(root, "migration-success.png") });
  await restart();
  assert(samePath((await invoke("get_storage_paths")).app_data, target), "new root must persist across another restart");
  const cancelled = path.join(root, `cancelled-${Date.now()}`);
  await invoke("change_data_directory", { directory: cancelled });
  await invoke("cancel_data_directory_change");
  assert.equal((await invoke("get_data_directory_change")).pending_directory, null);
  const failed = path.join(root, `occupied-${Date.now()}`);
  await invoke("change_data_directory", { directory: failed });
  await mkdir(failed);
  await writeFile(path.join(failed, "valuable.txt"), "do not overwrite");
  await restart();
  assert(samePath((await invoke("get_storage_paths")).app_data, target));
  assert((await invoke("get_data_directory_change")).message.includes("迁移失败"));
  assert.equal(await readFile(path.join(failed, "valuable.txt"), "utf8"), "do not overwrite");
  await storagePanel();
  await page.screenshot({ path: path.join(root, "migration-failed.png") });
  const result = { source, target, sourcePreserved: true, filesVerified: true, historyPreserved: true, pathsRelocated: true, persistsAcrossRestart: true, cancellationVerified: true, failureKeepsActiveDirectory: true };
  await writeFile(path.join(root, "migration-result.json"), JSON.stringify(result, null, 2));
  console.log(result);
} finally { await browser.close().catch(() => {}); }
