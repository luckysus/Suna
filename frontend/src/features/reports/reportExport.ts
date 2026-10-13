import { Document, HeadingLevel, ImageRun, Packer, Paragraph, Table, TableCell, TableRow, TextRun, WidthType } from "docx";
import PptxGenJS from "pptxgenjs";
import type { ReportChart } from "./reportCharts";

/**
 * 第 56 章：科研报告导出。
 *
 * 报告正文是 Markdown。三种产物全部本地生成、不联网：
 * - Word（.docx）：`docx` 包生成真正的 OOXML 文档，标题/正文/列表/表格对应转换；
 * - PPT（.pptx）：`pptxgenjs` 生成演示文稿，标题页 + 每个二级标题一页要点；
 * - PDF：渲染打印样式 HTML 到隐藏 iframe 并调用系统打印（Windows 自带
 *   "Microsoft Print to PDF"），矢量文字、中文完美，无需内嵌字体。
 *
 * 第 56 章的「图表」环节：数据集图表以自包含 SVG 生成，
 * PDF 内联 SVG、Word/PPT 嵌入栅格化后的 PNG。
 */

/** 已栅格化的报告图表（Word / PPT 用）。 */
export type ReportChartImage = {
  title: string;
  png: Uint8Array;
  width: number;
  height: number;
};

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunk = 0x8000;
  for (let index = 0; index < bytes.length; index += chunk) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunk));
  }
  return btoa(binary);
}

export type ReportBlock =
  | { kind: "h1" | "h2" | "h3" | "paragraph"; text: string }
  | { kind: "bullet"; text: string }
  | { kind: "table"; rows: string[][] };

/** 把报告 Markdown 解析为结构化块；表格解析连续的 `|` 行，`|---|` 分隔行被丢弃。 */
export function parseReportSections(markdown: string): ReportBlock[] {
  const blocks: ReportBlock[] = [];
  const lines = markdown.replace(/\r\n/g, "\n").split("\n");
  let index = 0;
  while (index < lines.length) {
    const line = lines[index].trim();
    if (!line) { index += 1; continue; }
    if (/^#{1,6}\s+/.test(line)) {
      const level = line.match(/^#+/)?.[0].length ?? 1;
      const text = line.replace(/^#+\s+/, "").trim();
      blocks.push({ kind: level === 1 ? "h1" : level === 2 ? "h2" : "h3", text });
      index += 1;
      continue;
    }
    if (/^\|.*\|\s*$/.test(line) || (line.startsWith("|") && line.includes("|"))) {
      const rows: string[][] = [];
      let cursor = index;
      while (cursor < lines.length) {
        const candidate = lines[cursor].trim();
        if (!candidate.startsWith("|")) break;
        if (!/^\|[\s:|-]+$/.test(candidate)) {
          rows.push(candidate.replace(/^\||\|$/g, "").split("|").map((cell) => cell.trim()));
        }
        cursor += 1;
      }
      if (rows.length >= 2) {
        blocks.push({ kind: "table", rows });
        index = cursor;
        continue;
      }
    }
    if (/^[-*+]\s+/.test(line)) {
      blocks.push({ kind: "bullet", text: line.replace(/^[-*+]\s+/, "").trim() });
      index += 1;
      continue;
    }
    if (/^\d+[.、)]\s*/.test(line)) {
      blocks.push({ kind: "bullet", text: line.replace(/^\d+[.、)]\s*/, "").trim() });
      index += 1;
      continue;
    }
    blocks.push({ kind: "paragraph", text: line.replace(/^>\s?/, "").replace(/[*_`]/g, "") });
    index += 1;
  }
  return blocks;
}

const escapeHtml = (value: string): string =>
  value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

/** 生成打印样式 HTML（用于系统打印导出 PDF）。 */
export function buildPrintHtml(markdown: string, title: string, charts: ReportChart[] = []): string {
  const blocks = parseReportSections(markdown);
  const body = blocks
    .map((block) => {
      const text = "text" in block ? escapeHtml(block.text) : "";
      switch (block.kind) {
        case "h1": return `<h1>${text}</h1>`;
        case "h2": return `<h2>${text}</h2>`;
        case "h3": return `<h3>${text}</h3>`;
        case "bullet": return `<li>${text}</li>`;
        case "table": {
          const [header, ...rows] = block.rows;
          const head = header.map((cell) => `<th>${escapeHtml(cell)}</th>`).join("");
          const bodyRows = rows
            .map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join("")}</tr>`)
            .join("");
          return `<table><thead><tr>${head}</tr></thead><tbody>${bodyRows}</tbody></table>`;
        }
        default: return `<p>${text}</p>`;
      }
    })
    .join("\n");
  const chartSection = charts.length
    ? `<section class="charts"><h2>数据与图表</h2>${charts
        .map((chart) => `<figure>${chart.svg}</figure>`)
        .join("")}</section>`
    : "";
  return `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><title>${escapeHtml(title)}</title><style>
    @page { margin: 20mm 18mm; }
    body { font-family: "Noto Sans SC", "Microsoft YaHei", "SimSun", sans-serif; color: #111827; font-size: 12pt; line-height: 1.7; }
    h1 { font-size: 20pt; border-bottom: 2px solid #2563EB; padding-bottom: 6px; }
    h2 { font-size: 16pt; color: #1E40AF; margin-top: 18pt; }
    h3 { font-size: 13pt; margin-top: 12pt; }
    table { border-collapse: collapse; width: 100%; margin: 8pt 0; font-size: 10.5pt; }
    th, td { border: 1px solid #9CA3AF; padding: 4pt 6pt; text-align: left; }
    th { background: #EFF6FF; }
    li { margin: 2pt 0; }
    .charts figure { margin: 10pt 0; page-break-inside: avoid; }
    .charts svg { max-width: 100%; height: auto; }
    footer { margin-top: 24pt; font-size: 9pt; color: #6B7280; border-top: 1px solid #E5E7EB; padding-top: 6pt; }
  </style></head><body><h1>${escapeHtml(title)}</h1>${body}${chartSection}<footer>由 Suna 生成 · ${new Date().toLocaleString("zh-CN")}</footer></body></html>`;
}

/** 调起系统打印（选择 "Microsoft Print to PDF" 即导出 PDF 文件）。 */
export function printReport(markdown: string, title: string, charts: ReportChart[] = []): void {
  const frame = document.createElement("iframe");
  frame.setAttribute("aria-hidden", "true");
  frame.style.position = "fixed";
  frame.style.width = "0";
  frame.style.height = "0";
  frame.style.border = "0";
  frame.srcdoc = buildPrintHtml(markdown, title, charts);
  frame.onload = () => {
    frame.contentWindow?.focus();
    frame.contentWindow?.print();
    window.setTimeout(() => frame.remove(), 60_000);
  };
  document.body.appendChild(frame);
}

/** 生成 Word 文档（.docx）。 */
export async function buildReportDocx(markdown: string, title: string, charts: ReportChartImage[] = []): Promise<Blob> {
  const blocks = parseReportSections(markdown);
  const children: Array<Paragraph | Table> = [new Paragraph({ text: title, heading: HeadingLevel.TITLE })];
  if (charts.length > 0) {
    children.push(new Paragraph({ text: "数据与图表", heading: HeadingLevel.HEADING_1 }));
    const maxWidth = 600;
    for (const chart of charts) {
      const ratio = chart.height / chart.width;
      children.push(
        new Paragraph({
          children: [
            new ImageRun({
              type: "png",
              data: chart.png,
              transformation: { width: maxWidth, height: Math.round(maxWidth * ratio) },
            }),
          ],
        }),
      );
    }
  }
  let listBuffer: Paragraph[] = [];
  const flushList = () => {
    children.push(...listBuffer);
    listBuffer = [];
  };
  for (const block of blocks) {
    switch (block.kind) {
      case "h1":
        flushList();
        children.push(new Paragraph({ text: block.text, heading: HeadingLevel.HEADING_1 }));
        break;
      case "h2":
        flushList();
        children.push(new Paragraph({ text: block.text, heading: HeadingLevel.HEADING_2 }));
        break;
      case "h3":
        flushList();
        children.push(new Paragraph({ text: block.text, heading: HeadingLevel.HEADING_3 }));
        break;
      case "bullet":
        listBuffer.push(new Paragraph({ text: block.text, bullet: { level: 0 } }));
        break;
      case "table": {
        flushList();
        const [header, ...rows] = block.rows;
        children.push(new Table({
          width: { size: 100, type: WidthType.PERCENTAGE },
          rows: [
            new TableRow({ children: header.map((cell) => new TableCell({ children: [new Paragraph({ children: [new TextRun({ text: cell, bold: true })] })] })) }),
            ...rows.map((row) => new TableRow({ children: row.map((cell) => new TableCell({ children: [new Paragraph({ text: cell })] })) })),
          ],
        }));
        break;
      }
      default:
        flushList();
        children.push(new Paragraph({ children: [new TextRun({ text: block.text, size: 24 })] }));
        break;
    }
  }
  flushList();
  const document = new Document({
    styles: {
      default: {
        document: { run: { font: "Microsoft YaHei", size: 24 } },
      },
    },
    sections: [{ children }],
  });
  return Packer.toBlob(document);
}

/** 生成 PPT 演示文稿（.pptx）：标题页 + 图表页 + 每个二级标题一页要点。 */
export async function buildReportPptx(markdown: string, title: string, charts: ReportChartImage[] = []): Promise<Blob> {
  const blocks = parseReportSections(markdown);
  const pptx = new PptxGenJS();
  pptx.title = title;
  pptx.layout = "LAYOUT_16x9";

  const cover = pptx.addSlide();
  cover.addText(title, { x: 0.6, y: 2.0, w: 8.8, h: 1.2, fontSize: 32, bold: true, color: "1D4ED8" });
  cover.addText("Suna · AI for Steel Research", { x: 0.6, y: 3.3, w: 8.8, h: 0.5, fontSize: 14, color: "6B7280" });

  for (const chart of charts) {
    const slide = pptx.addSlide();
    slide.addText(chart.title, { x: 0.6, y: 0.35, w: 8.8, h: 0.7, fontSize: 20, bold: true, color: "1D4ED8" });
    const width = 8.8;
    const height = Math.min(width * (chart.height / chart.width), 4.1);
    slide.addImage({ data: `image/png;base64,${bytesToBase64(chart.png)}`, x: 0.6, y: 1.25, w: width, h: height });
  }

  const sections = new Map<string, string[]>();
  let current = "摘要";
  for (const block of blocks) {
    if (block.kind === "h1" || block.kind === "h2") {
      current = block.text;
      sections.set(current, sections.get(current) ?? []);
      continue;
    }
    if (block.kind === "paragraph" && !sections.has(current)) continue;
    const lines = sections.get(current) ?? [];
    if (block.kind === "bullet") {
      lines.push(`• ${block.text}`);
    } else if (block.kind === "paragraph" && block.text.length > 0) {
      lines.push(block.text.length > 110 ? `${block.text.slice(0, 110)}…` : block.text);
    } else if (block.kind === "table") {
      lines.push(block.rows.map((row) => row.join(" | ")).join("\n"));
    }
    sections.set(current, lines);
  }

  for (const [heading, lines] of sections) {
    if (lines.length === 0) continue;
    const slide = pptx.addSlide();
    slide.addText(heading, { x: 0.6, y: 0.35, w: 8.8, h: 0.8, fontSize: 22, bold: true, color: "1D4ED8" });
    slide.addText(lines.slice(0, 8).join("\n"), {
      x: 0.6, y: 1.25, w: 8.8, h: 5.4, fontSize: 13, color: "111827", valign: "top", lineSpacingMultiple: 1.3,
    });
  }
  const blob = (await pptx.write({ outputType: "blob" })) as Blob;
  return blob;
}

/** 通过保存对话框把 Blob 写盘；返回用户是否完成了保存。 */
export async function saveReportBlob(
  blob: Blob,
  topic: string,
  extension: string,
  description: string,
): Promise<boolean> {
  const safeTopic = topic.replace(/[\\/:*?"<>|]/g, "_");
  const { desktop } = await import("../../bridge/desktop");
  const buffer = typeof blob.arrayBuffer === "function" ? await blob.arrayBuffer() : await new Promise<ArrayBuffer>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as ArrayBuffer);
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(blob);
  });
  const saved = await desktop.saveExportFile(`${safeTopic}.${extension}`, Array.from(new Uint8Array(buffer)));
  return saved !== null;
}
