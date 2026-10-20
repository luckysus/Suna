#!/usr/bin/env node
/**
 * Download WeKnora catalog sources into .weknora-upstream/ then run the sync.
 * Requires network access to raw.githubusercontent.com / api.github.com.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const base = "https://raw.githubusercontent.com/Tencent/WeKnora/main";
const upstream = path.join(root, ".weknora-upstream");

async function download(url, dest) {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status} ${url}`);
  const buf = Buffer.from(await res.arrayBuffer());
  fs.writeFileSync(dest, buf);
  console.log("ok", path.relative(root, dest), buf.length);
}

async function main() {
  const files = [
    ["internal/models/catalog/data/models.generated.json", "catalog/models.generated.json"],
    ["internal/models/catalog/data/seed.json", "catalog/seed.json"],
    ["internal/models/catalog/data/overrides.json", "catalog/overrides.json"],
    ["internal/models/legacy_keys.go", "catalog/legacy_keys.go"],
  ];
  for (const [remote, local] of files) {
    await download(`${base}/${remote}`, path.join(upstream, local));
  }

  const providersApi =
    "https://api.github.com/repos/Tencent/WeKnora/contents/internal/models/providers?ref=main";
  const providers = await (await fetch(providersApi)).json();
  for (const entry of providers) {
    if (entry.type !== "file" || !entry.name.endsWith(".go")) continue;
    if (entry.name.includes("_test")) continue;
    await download(entry.download_url, path.join(upstream, "providers", entry.name));
  }

  const assetsApi =
    "https://api.github.com/repos/Tencent/WeKnora/contents/internal/models/providers/assets?ref=main";
  const assets = await (await fetch(assetsApi)).json();
  for (const entry of assets) {
    if (entry.type !== "file") continue;
    await download(entry.download_url, path.join(upstream, "assets", entry.name));
  }

  const sync = spawnSync(process.execPath, [path.join(__dirname, "sync-weknora-model-catalog.mjs")], {
    cwd: root,
    stdio: "inherit",
  });
  process.exit(sync.status ?? 1);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
