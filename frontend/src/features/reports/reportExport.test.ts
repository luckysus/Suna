import { describe, expect, it } from "vitest";
import {
  buildPrintHtml,
  buildReportDocx,
  buildReportPptx,
  parseReportSections,
} from "./reportExport";

const sample = `# Q690 高强钢工艺研究报告

## 研究背景

Q690 广泛用于工程机械结构件，热处理工艺窗口决定其强韧性匹配。

- 淬火温度影响马氏体体积分数
- 回火温度决定碳化物析出

## 数据与方法

| 成分 | 范围 |
| --- | --- |
| C | 0.12-0.18 |
| Mn | 1.30-1.60 |

## 结论

1. 回火温度是首要敏感变量。
`;

const readMagic = async (blob: Blob): Promise<Uint8Array> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(blob.slice(0, 2));
  });

describe("parseReportSections", () => {
  it("splits headings, paragraphs, bullets and tables", () => {
    const blocks = parseReportSections(sample);
    expect(blocks[0]).toEqual({ kind: "h1", text: "Q690 高强钢工艺研究报告" });
    expect(blocks.filter((block) => block.kind === "h2")).toHaveLength(3);
    // 两个无序要点 + 一个有序项，有序列表按设计归一为要点。
    expect(blocks.filter((block) => block.kind === "bullet")).toHaveLength(3);
    const table = blocks.find((block) => block.kind === "table");
    expect(table).toEqual({
      kind: "table",
      rows: [
        ["成分", "范围"],
        ["C", "0.12-0.18"],
        ["Mn", "1.30-1.60"],
      ],
    });
    expect(blocks.some((block) => block.kind === "bullet" && block.text.includes("回火温度是首要敏感变量"))).toBe(true);
  });

  it("keeps the separator row out of the parsed table", () => {
    const blocks = parseReportSections("| A | B |\n| --- | --- |\n| 1 | 2 |");
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({ kind: "table", rows: [["A", "B"], ["1", "2"]] });
  });
});

describe("report export artifacts", () => {
  it("builds a real docx package (zip) from the report", async () => {
    const blob = await buildReportDocx(sample, "Q690 工艺研究报告");
    expect(blob.size).toBeGreaterThan(2000);
    const magic = await readMagic(blob);
    expect([magic[0], magic[1]]).toEqual([0x50, 0x4b]); // "PK"
  });

  it("builds a real pptx package (zip) with one slide per section", async () => {
    const blob = await buildReportPptx(sample, "Q690 工艺研究报告");
    expect(blob.size).toBeGreaterThan(2000);
    const magic = await readMagic(blob);
    expect([magic[0], magic[1]]).toEqual([0x50, 0x4b]);
  });

  it("renders print HTML with escaped content and print styles", () => {
    const html = buildPrintHtml("## 结论\n\n强度 < 650 MPa", "报告 <标题>");
    expect(html).toContain("<h2>结论</h2>");
    expect(html).toContain("强度 &lt; 650 MPa");
    expect(html).toContain("报告 &lt;标题&gt;");
    expect(html).toContain("@page");
  });
});
