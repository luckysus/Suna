import { describe, expect, it } from "vitest";
import type { DatasetAnalysis, DatasetColumnAnalysis } from "../../bridge/desktop";
import {
  buildBarChartSvg,
  buildCorrelationHeatmapSvg,
  buildReportCharts,
  correlationMatrix,
} from "./reportCharts";

function column(overrides: Partial<DatasetColumnAnalysis> & { ordinal: number; name: string }): DatasetColumnAnalysis {
  return {
    canonicalField: null,
    unit: null,
    inferredType: "number",
    sampleCount: 10,
    missingCount: 0,
    invalidCount: 0,
    missingRate: 0,
    distinctCount: 10,
    mean: 0,
    standardDeviation: 1,
    min: 0,
    percentile25: 0,
    median: 0,
    percentile75: 0,
    max: 0,
    outlierCount: 0,
    outlierRows: [],
    topValues: [],
    distribution: [],
    ...overrides,
  };
}

function analysis(overrides: Partial<DatasetAnalysis> = {}): DatasetAnalysis {
  return {
    datasetId: "dataset-1",
    sourceSha256: null,
    selectedSheet: null,
    rowCount: 10,
    analyzedRowCount: 10,
    excludedRowCount: 0,
    columns: [],
    groups: [],
    correlations: [],
    warnings: [],
    ...overrides,
  };
}

describe("buildBarChartSvg", () => {
  it("为每个字段画一根柱子并带上标题", () => {
    const svg = buildBarChartSvg(
      [
        { label: "C", value: 0.15 },
        { label: "Mn", value: 1.42 },
      ],
      "字段均值",
    );
    expect(svg).toContain("<svg");
    expect(svg).toContain("字段均值");
    // 两根柱子 + 两个轴标签
    expect(svg.match(/<rect /g)?.length).toBe(3); // 背景 + 2 根柱子
    expect(svg).toContain("0.15");
    expect(svg).toContain("1.42");
  });

  it("转义字段名里的 XML 特殊字符", () => {
    const svg = buildBarChartSvg([{ label: "a<b>&c", value: 1 }], "t");
    expect(svg).toContain("a&lt;b&gt;&amp;c");
    expect(svg).not.toContain("a<b>&c");
  });
});

describe("buildCorrelationHeatmapSvg", () => {
  it("按矩阵维度画出对称网格", () => {
    const svg = buildCorrelationHeatmapSvg(
      ["C", "Mn", "Cr"],
      [
        [1, 0.6, -0.3],
        [0.6, 1, 0.1],
        [-0.3, 0.1, 1],
      ],
      "字段相关性",
    );
    expect(svg).toContain("字段相关性");
    // 9 个单元格（含对角线）
    expect(svg.match(/<rect /g)?.length).toBe(10); // 背景 + 9 格
    expect(svg).toContain("0.60");
    expect(svg).toContain("-0.30");
  });
});

describe("correlationMatrix", () => {
  it("从相关系数还原对称矩阵，对角线为 1", () => {
    const matrix = correlationMatrix(
      analysis({
        columns: [column({ ordinal: 0, name: "C" }), column({ ordinal: 1, name: "Mn" })],
        correlations: [{ leftOrdinal: 0, rightOrdinal: 1, sampleCount: 10, pearson: 0.72 }],
      }),
    );
    expect(matrix).not.toBeNull();
    expect(matrix?.labels).toEqual(["C", "Mn"]);
    expect(matrix?.matrix[0][0]).toBe(1);
    expect(matrix?.matrix[0][1]).toBeCloseTo(0.72);
    expect(matrix?.matrix[1][0]).toBeCloseTo(0.72);
  });

  it("字段不足两个时返回 null", () => {
    const matrix = correlationMatrix(
      analysis({
        columns: [column({ ordinal: 0, name: "C" })],
        correlations: [{ leftOrdinal: 0, rightOrdinal: 0, sampleCount: 10, pearson: 1 }],
      }),
    );
    expect(matrix).toBeNull();
  });
});

describe("buildReportCharts", () => {
  it("同时产出字段均值柱状图与相关性热力图", () => {
    const charts = buildReportCharts(
      analysis({
        columns: [
          column({ ordinal: 0, name: "C", mean: 0.15, unit: "%" }),
          column({ ordinal: 1, name: "Mn", mean: 1.4 }),
        ],
        correlations: [{ leftOrdinal: 0, rightOrdinal: 1, sampleCount: 10, pearson: 0.5 }],
      }),
      "Q690",
    );
    expect(charts.map((chart) => chart.id)).toEqual(["column-means", "correlation"]);
    expect(charts[0].title).toContain("Q690");
    expect(charts[0].svg).toContain("C(%)");
  });

  it("没有相关性数据时只出柱状图", () => {
    const charts = buildReportCharts(
      analysis({ columns: [column({ ordinal: 0, name: "C", mean: 0.15 })] }),
      "Q690",
    );
    expect(charts.map((chart) => chart.id)).toEqual(["column-means"]);
  });

  it("没有数值字段时不出图", () => {
    const charts = buildReportCharts(
      analysis({ columns: [column({ ordinal: 0, name: "牌号", inferredType: "text", mean: null })] }),
      "Q690",
    );
    expect(charts).toEqual([]);
  });
});
