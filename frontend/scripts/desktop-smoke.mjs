// Run against a development Tauri window started with WebView2 CDP enabled.
// node scripts/desktop-smoke.mjs --output <directory> [--baseline <directory>] [--update-baseline]
import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";

const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const output = path.resolve(option("--output", "desktop-smoke-results"));
const baseline = option("--baseline");
const update = args.includes("--update-baseline");
assert(!update || baseline, "--update-baseline requires --baseline");
await mkdir(output, { recursive: true });
if (update) await mkdir(baseline, { recursive: true });
const browser = await chromium.connectOverCDP(option("--cdp", "http://127.0.0.1:9223"));
const page = browser.contexts().flatMap((context) => context.pages()).find((item) => item.url().includes("127.0.0.1:1420"));
assert(page, "development Tauri window not found");
assert(await page.evaluate(() => !!window.__TAURI_INTERNALS__), "requires a real Tauri WebView");
const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
const report = { runtime: "Windows Tauri / WebView2", pages: [], charts: [], errors };
const previousTheme = await page.evaluate(async () => (await import("/src/stores/settingsStore.ts")).useSettingsStore.getState().themePreference);
async function capture(name) {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(450);
  const layout = await page.evaluate(() => ({
    width: innerWidth, height: innerHeight, ratio: devicePixelRatio,
    horizontalOverflow: document.documentElement.scrollWidth > innerWidth + 1,
    headings: [...document.querySelectorAll("h1,h2")].map((item) => item.textContent),
  }));
  assert(!layout.horizontalOverflow, `${name}: viewport overflow`);
  if (name.endsWith("-chat")) {
    const composer = await page.locator(".suna-chat-composer").boundingBox();
    assert(composer && composer.y + composer.height <= layout.height + 1, "chat composer must remain inside the desktop window");
  }
  const png = await page.screenshot({ animations: "disabled", caret: "hide" });
  await writeFile(path.join(output, `${name}.png`), png);
  let mismatch = null;
  if (baseline && update) await writeFile(path.join(baseline, `${name}.png`), png);
  else if (baseline) {
    const expected = await readFile(path.join(baseline, `${name}.png`));
    const comparison = await page.evaluate(async ({ actual, expected }) => {
      const decode = async (base64) => {
        const image = new Image();
        image.src = `data:image/png;base64,${base64}`;
        await image.decode();
        const canvas = document.createElement("canvas");
        canvas.width = image.width; canvas.height = image.height;
        const context = canvas.getContext("2d");
        context.drawImage(image, 0, 0);
        return { width: image.width, height: image.height, pixels: context.getImageData(0, 0, image.width, image.height).data };
      };
      const a = await decode(actual), b = await decode(expected);
      if (a.width !== b.width || a.height !== b.height) return { sameSize: false, mismatch: 1 };
      let changed = 0;
      for (let i = 0; i < a.pixels.length; i += 4) {
        if ([0, 1, 2].some((channel) => Math.abs(a.pixels[i + channel] - b.pixels[i + channel]) > 32)) changed++;
      }
      return { sameSize: true, mismatch: changed / (a.width * a.height) };
    }, { actual: png.toString("base64"), expected: expected.toString("base64") });
    mismatch = comparison.mismatch;
    assert(comparison.sameSize && mismatch <= 0.005, `${name}: screenshot mismatch ${mismatch}`);
  }
  report.pages.push({ name, ...layout, mismatch });
}
try {
  const routes = await page.evaluate(async () => Object.entries((await import("/src/app/sectionRoute.ts")).SECTION_PATHS));
  for (const theme of ["light", "dark"]) {
    await page.evaluate(async (theme) => (await import("/src/stores/settingsStore.ts")).useSettingsStore.getState().setThemePreference(theme), theme);
    for (const [section, route] of routes) {
      await page.evaluate((route) => { history.pushState(null, "", route); dispatchEvent(new PopStateEvent("popstate")); }, route);
      await capture(`${theme}-${section}`);
    }
  }
  const artifacts = await page.evaluate(async () => {
    const charts = await import("/src/features/reports/reportCharts.ts");
    const exports = await import("/src/features/reports/reportExport.ts");
    const bar = charts.buildBarChartSvg([{ label: "屈服强度", value: 355 }, { label: "温度偏差", value: -30 }], "真实桌面中文图表");
    const heatmap = charts.buildCorrelationHeatmapSvg(["强度", "温度"], [[1, -0.8], [-0.8, 1]], "相关性");
    const images = [];
    const result = [];
    for (const [name, svg, width, height] of [["bar", bar, 720, 320], ["heatmap", heatmap, 720, 190]]) {
      const png = await charts.svgToPng(svg, width, height);
      const image = new Image();
      image.src = URL.createObjectURL(new Blob([png], { type: "image/png" }));
      await image.decode();
      const canvas = document.createElement("canvas"); canvas.width = image.width; canvas.height = image.height;
      const context = canvas.getContext("2d"); context.drawImage(image, 0, 0);
      const pixels = context.getImageData(0, 0, image.width, image.height).data;
      let colored = 0;
      for (let i = 0; i < pixels.length; i += 4) if (pixels[i] < 240 || pixels[i + 1] < 240 || pixels[i + 2] < 240) colored++;
      URL.revokeObjectURL(image.src);
      result.push({ name: `${name}.png`, bytes: Array.from(png), width: image.width, height: image.height, colored });
      images.push({ title: name, png, width, height });
    }
    const markdown = "# 真实桌面报告\n## 结果\n- 中文图表与负数\n";
    const docx = await exports.buildReportDocx(markdown, "Suna 桌面验证", images);
    const pptx = await exports.buildReportPptx(markdown, "Suna 桌面验证", images);
    result.push({ name: "report.docx", bytes: Array.from(new Uint8Array(await docx.arrayBuffer())) });
    result.push({ name: "report.pptx", bytes: Array.from(new Uint8Array(await pptx.arrayBuffer())) });
    return result;
  });
  for (const { bytes, ...artifact } of artifacts) {
    const buffer = Buffer.from(bytes);
    if (artifact.name.endsWith(".png")) {
      assert.equal(buffer.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
      assert(artifact.width === 1440 && artifact.colored > 1000, "rasterized chart must contain visible pixels");
    } else assert.equal(buffer.subarray(0, 2).toString(), "PK");
    await writeFile(path.join(output, artifact.name), buffer);
    report.charts.push({ ...artifact, bytes: buffer.length });
  }
  assert.deepEqual(errors, [], "desktop page errors");
  console.log(`Verified ${report.pages.length} desktop views and ${report.charts.length} chart/export artifacts.`);
} finally {
  await writeFile(path.join(output, "results.json"), JSON.stringify(report, null, 2));
  await page.evaluate(async (theme) => (await import("/src/stores/settingsStore.ts")).useSettingsStore.getState().setThemePreference(theme), previousTheme).catch(() => {});
  await browser.close();
}
