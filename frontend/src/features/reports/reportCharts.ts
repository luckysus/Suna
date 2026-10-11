import type { DatasetAnalysis } from "../../bridge/desktop";

/**
 * 第 56 章：科研报告图表。
 *
 * 报告里的图表用**自包含 SVG** 生成（显式颜色，不引用 CSS 变量），
 * 因此同一份产物可以三处复用：
 * - 预览：直接内联到 DOM；
 * - PDF：内联进打印 HTML（浏览器原生渲染，中文清晰）；
 * - Word / PPT：先经 {@link svgToPng} 栅格化为 PNG 再嵌入（docx 的 SVG 也要求 PNG fallback）。
 */

export type ReportChart = {
  id: string;
  title: string;
  svg: string;
  width: number;
  height: number;
};

const CHART_WIDTH = 720;
const CHART_HEIGHT = 320;

const COLORS = {
  axis: "#9CA3AF",
  grid: "#E5E7EB",
  bar: "#2563EB",
  barSoft: "#93B4F5",
  text: "#111827",
  muted: "#6B7280",
  empty: "#F1F5F9",
};

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function shortLabel(value: string, max = 8): string {
  return value.length > max ? `${value.slice(0, max - 1)}…` : value;
}

function formatNumber(value: number): string {
  if (!Number.isFinite(value)) return "-";
  if (value !== 0 && Math.abs(value) < 0.01) return value.toExponential(1);
  if (Math.abs(value) >= 1000) return value.toFixed(0);
  return value.toFixed(Math.abs(value) >= 100 ? 0 : 2);
}

/** 字段均值柱状图。 */
export function buildBarChartSvg(items: Array<{ label: string; value: number }>, title: string): string {
  const margin = { top: 46, right: 20, bottom: 76, left: 68 };
  const plotWidth = CHART_WIDTH - margin.left - margin.right;
  const plotHeight = CHART_HEIGHT - margin.top - margin.bottom;
  const values = items.map((item) => item.value);
  const maxValue = Math.max(...values, 0);
  const minValue = Math.min(...values, 0);
  const span = maxValue - minValue || 1;
  const scaleY = (value: number) => margin.top + plotHeight - ((value - minValue) / span) * plotHeight;
  const slot = plotWidth / Math.max(items.length, 1);
  const barWidth = Math.max(Math.min(slot * 0.62, 54), 6);

  const ticks = [0, 0.25, 0.5, 0.75, 1]
    .map((ratio) => minValue + span * ratio)
    .map((value) => {
      const y = scaleY(value);
      return `<line x1="${margin.left}" y1="${y.toFixed(1)}" x2="${(CHART_WIDTH - margin.right).toFixed(1)}" y2="${y.toFixed(1)}" stroke="${COLORS.grid}" stroke-width="1"/>
        <text x="${(margin.left - 8).toFixed(1)}" y="${(y + 3.5).toFixed(1)}" font-size="10" fill="${COLORS.muted}" text-anchor="end">${escapeXml(formatNumber(value))}</text>`;
    })
    .join("");

  const zeroY = scaleY(0);
  const bars = items
    .map((item, index) => {
      const center = margin.left + slot * index + slot / 2;
      const top = scaleY(Math.max(item.value, 0));
      const bottom = scaleY(Math.min(item.value, 0));
      const height = Math.max(bottom - top, 1);
      const labelY = CHART_HEIGHT - margin.bottom + 16;
      return `<rect x="${(center - barWidth / 2).toFixed(1)}" y="${top.toFixed(1)}" width="${barWidth.toFixed(1)}" height="${height.toFixed(1)}" fill="${COLORS.bar}" rx="2"/>
        <text x="${center.toFixed(1)}" y="${(top - 6).toFixed(1)}" font-size="10" fill="${COLORS.text}" text-anchor="middle">${escapeXml(formatNumber(item.value))}</text>
        <text x="${center.toFixed(1)}" y="${labelY}" font-size="10" fill="${COLORS.muted}" text-anchor="end" transform="rotate(-32 ${center.toFixed(1)} ${labelY})">${escapeXml(shortLabel(item.label))}</text>`;
    })
    .join("");

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${CHART_WIDTH}" height="${CHART_HEIGHT}" viewBox="0 0 ${CHART_WIDTH} ${CHART_HEIGHT}" role="img" aria-label="${escapeXml(title)}">
  <rect width="${CHART_WIDTH}" height="${CHART_HEIGHT}" fill="#FFFFFF"/>
  <text x="${margin.left}" y="24" font-size="14" font-weight="600" fill="${COLORS.text}">${escapeXml(title)}</text>
  ${ticks}
  <line x1="${margin.left}" y1="${zeroY.toFixed(1)}" x2="${(CHART_WIDTH - margin.right).toFixed(1)}" y2="${zeroY.toFixed(1)}" stroke="${COLORS.axis}" stroke-width="1"/>
  ${bars}
</svg>`;
}

function correlationColor(value: number | null): { fill: string; text: string } {
  if (value == null || !Number.isFinite(value)) return { fill: COLORS.empty, text: COLORS.muted };
  const clamped = Math.max(-1, Math.min(1, value));
  const magnitude = Math.abs(clamped);
  const base = clamped >= 0 ? [239, 68, 68] : [37, 99, 235];
  const strength = 0.12 + 0.78 * magnitude;
  const mix = (channel: number) => Math.round(255 - (255 - channel) * strength);
  return {
    fill: `rgb(${mix(base[0])}, ${mix(base[1])}, ${mix(base[2])})`,
    text: magnitude > 0.55 ? "#FFFFFF" : COLORS.text,
  };
}

/** 相关性热力图（对称矩阵）。 */
export function buildCorrelationHeatmapSvg(labels: string[], matrix: Array<Array<number | null>>, title: string): string {
  const size = labels.length;
  const originX = 132;
  const originY = 64;
  const available = CHART_WIDTH - originX - 24;
  const cell = Math.max(Math.min(available / size, 46), 12);
  const gridSize = cell * size;
  const height = originY + gridSize + 34;

  const cells: string[] = [];
  for (let row = 0; row < size; row += 1) {
    for (let column = 0; column < size; column += 1) {
      const value = matrix[row]?.[column] ?? null;
      const { fill, text } = correlationColor(value);
      const x = originX + column * cell;
      const y = originY + row * cell;
      cells.push(`<rect x="${x.toFixed(1)}" y="${y.toFixed(1)}" width="${cell.toFixed(1)}" height="${cell.toFixed(1)}" fill="${fill}" stroke="#FFFFFF" stroke-width="0.5"/>`);
      if (cell >= 26) {
        cells.push(`<text x="${(x + cell / 2).toFixed(1)}" y="${(y + cell / 2 + 3.5).toFixed(1)}" font-size="9" fill="${text}" text-anchor="middle">${value == null ? "" : value.toFixed(2)}</text>`);
      }
    }
  }

  const rowLabels = labels
    .map((label, index) => {
      const y = originY + index * cell + cell / 2 + 3.5;
      return `<text x="${(originX - 8).toFixed(1)}" y="${y.toFixed(1)}" font-size="10" fill="${COLORS.muted}" text-anchor="end">${escapeXml(shortLabel(label, 12))}</text>`;
    })
    .join("");
  const columnLabels = labels
    .map((label, index) => {
      const x = originX + index * cell + cell / 2;
      const y = originY + gridSize + 14;
      return `<text x="${x.toFixed(1)}" y="${y.toFixed(1)}" font-size="10" fill="${COLORS.muted}" text-anchor="end" transform="rotate(-32 ${x.toFixed(1)} ${y.toFixed(1)})">${escapeXml(shortLabel(label, 12))}</text>`;
    })
    .join("");

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${CHART_WIDTH}" height="${Math.round(height)}" viewBox="0 0 ${CHART_WIDTH} ${Math.round(height)}" role="img" aria-label="${escapeXml(title)}">
  <rect width="${CHART_WIDTH}" height="${Math.round(height)}" fill="#FFFFFF"/>
  <text x="24" y="24" font-size="14" font-weight="600" fill="${COLORS.text}">${escapeXml(title)}</text>
  <text x="24" y="42" font-size="10" fill="${COLORS.muted}">蓝=负相关，红=正相关，越深越强</text>
  ${cells.join("")}
  ${rowLabels}
  ${columnLabels}
</svg>`;
}

/** 从数据集分析结果取出相关性矩阵；样本不足时返回 null。 */
export function correlationMatrix(
  analysis: DatasetAnalysis,
): { labels: string[]; matrix: Array<Array<number | null>> } | null {
  const ordinals = new Set<number>();
  for (const correlation of analysis.correlations) {
    ordinals.add(correlation.leftOrdinal);
    ordinals.add(correlation.rightOrdinal);
  }
  const ordered = [...ordinals].sort((left, right) => left - right);
  if (ordered.length < 2) return null;
  const position = new Map(ordered.map((ordinal, index) => [ordinal, index]));
  const size = ordered.length;
  const matrix: Array<Array<number | null>> = Array.from({ length: size }, () =>
    Array.from({ length: size }, () => null as number | null),
  );
  for (let index = 0; index < size; index += 1) matrix[index][index] = 1;
  for (const correlation of analysis.correlations) {
    const row = position.get(correlation.leftOrdinal);
    const column = position.get(correlation.rightOrdinal);
    if (row == null || column == null) continue;
    matrix[row][column] = correlation.pearson;
    matrix[column][row] = correlation.pearson;
  }
  const labels = ordered.map(
    (ordinal) => analysis.columns.find((column) => column.ordinal === ordinal)?.name ?? `列 ${ordinal}`,
  );
  return { labels, matrix };
}

/** 依据数据集分析结果生成报告用图表：字段均值柱状图 + 相关性热力图。 */
export function buildReportCharts(analysis: DatasetAnalysis, sourceName: string): ReportChart[] {
  const charts: ReportChart[] = [];
  const numeric = analysis.columns
    .filter((column) => column.inferredType === "number" && column.mean != null)
    .slice(0, 12);
  if (numeric.length > 0) {
    const items = numeric.map((column) => ({
      label: column.unit ? `${column.name}(${column.unit})` : column.name,
      value: column.mean as number,
    }));
    charts.push({
      id: "column-means",
      title: `${sourceName} · 字段均值`,
      svg: buildBarChartSvg(items, `${sourceName} · 字段均值`),
      width: CHART_WIDTH,
      height: CHART_HEIGHT,
    });
  }
  const matrix = correlationMatrix(analysis);
  if (matrix) {
    const title = `${sourceName} · 字段相关性`;
    const svg = buildCorrelationHeatmapSvg(matrix.labels, matrix.matrix, title);
    const height = Number(svg.match(/height="(\d+)"/)?.[1] ?? CHART_HEIGHT);
    charts.push({ id: "correlation", title, svg, width: CHART_WIDTH, height });
  }
  return charts;
}

/** 把自包含 SVG 栅格化为 PNG（Word / PPT 需要位图；docx 的 SVG 也要求 PNG fallback）。 */
export async function svgToPng(svg: string, width: number, height: number, scale = 2): Promise<Uint8Array> {
  const blob = new Blob([svg], { type: "image/svg+xml;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const element = new Image();
      element.onload = () => resolve(element);
      element.onerror = () => reject(new Error("图表渲染失败"));
      element.src = url;
    });
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(width * scale);
    canvas.height = Math.round(height * scale);
    const context = canvas.getContext("2d");
    if (!context) throw new Error("当前环境不支持图表导出");
    context.fillStyle = "#FFFFFF";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const buffer = await new Promise<ArrayBuffer>((resolve, reject) => {
      canvas.toBlob((result) => {
        if (!result) { reject(new Error("图表编码失败")); return; }
        void result.arrayBuffer().then(resolve);
      }, "image/png");
    });
    return new Uint8Array(buffer);
  } finally {
    URL.revokeObjectURL(url);
  }
}
