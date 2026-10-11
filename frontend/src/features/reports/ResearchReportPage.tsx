import { useEffect, useState } from "react";
import { BarChart3, Download, FileChartColumn, FileText, Loader2, Presentation, Printer, Save, Sparkles } from "lucide-react";
import { desktop, type DatasetAnalysis, type ReportRecord, type SteelDatasetRecord } from "../../bridge/desktop";
import "./reports.css";
import { Textarea } from "../../components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../components/ui/select";
import { buildReportDocx, buildReportPptx, printReport, saveReportBlob, type ReportChartImage } from "./reportExport";
import { buildReportCharts, svgToPng, type ReportChart } from "./reportCharts";
import { errorMessage } from "../../utils/errorMessage";
import { Button } from "../../components/ui/button";
import { SavedReportsPanel } from "./SavedReportsPanel";

/**
 * 第 56 章：科研报告。
 *
 * 报告类型：研究报告 / 实验报告 / 文献综述 / 技术报告。
 * 生成过程：数据 → 分析 → 图表 → 结论 → 引用 → 报告。
 * 导出：Markdown / Word / PPT / PDF；第 80 章要求报告落库（`reports` 表）。
 */

const REPORT_TYPES = ["研究报告", "实验报告", "文献综述", "技术报告"];

export default function ResearchReportPage() {
  const [type, setType] = useState("研究报告");
  const [topic, setTopic] = useState("");
  const [content, setContent] = useState("");
  const [busy, setBusy] = useState(false);
  const [exporting, setExporting] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [datasets, setDatasets] = useState<SteelDatasetRecord[]>([]);
  const [datasetId, setDatasetId] = useState("");
  const [charts, setCharts] = useState<ReportChart[]>([]);
  const [saved, setSaved] = useState<ReportRecord[]>([]);
  const [saving, setSaving] = useState(false);

  const loadSaved = async () => {
    try {
      setSaved(await desktop.listReports());
    } catch (reason) {
      setError(errorMessage(reason, "无法读取已保存的报告"));
    }
  };
  useEffect(() => { void loadSaved(); }, []);

  const saveReport = async () => {
    if (!content.trim()) return;
    setSaving(true); setError("");
    try {
      const record = await desktop.saveReport({
        kind: type,
        title: (topic.trim() || type).slice(0, 200),
        topic: topic.trim(),
        format: "markdown",
        contentMarkdown: content,
        datasetId: datasetId || null,
        knowledgeBaseIds: [],
        metadata: { charts: charts.length, datasetName: datasets.find((item) => item.id === datasetId)?.sourceName ?? null },
      });
      setNotice(`报告已保存（${record.kind}）`);
      await loadSaved();
    } catch (reason) {
      setError(errorMessage(reason, "报告保存失败"));
    } finally {
      setSaving(false);
    }
  };

  const openSaved = (report: ReportRecord) => {
    setType(report.kind);
    setTopic(report.topic || report.title);
    setContent(report.content_markdown);
    setNotice(`已打开「${report.title}」`);
  };

  const removeSaved = async (report: ReportRecord) => {
    try {
      await desktop.deleteReport(report.id);
      setNotice(`已删除「${report.title}」`);
      await loadSaved();
    } catch (reason) {
      setError(errorMessage(reason, "删除报告失败"));
    }
  };


  // 可选数据来源：选中后在报告里生成「数据与图表」。
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const items = await desktop.listSteelDatasets();
        if (!cancelled) setDatasets(items.filter((item) => item.mappingState === "ready"));
      } catch {
        // 数据集不可用时报告仍可只靠 Agent 生成。
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // 第 56 章「数据 → 分析 → 图表」：选中数据集即产出图表（字段均值 + 相关性）。
  useEffect(() => {
    let cancelled = false;
    const dataset = datasets.find((item) => item.id === datasetId);
    if (!dataset) { setCharts([]); return; }
    void (async () => {
      try {
        const correlationColumns = dataset.columns
          .filter((column) => column.inferredType === "number" && !column.duplicate)
          .map((column) => column.ordinal)
          .slice(0, 12);
        const analysis: DatasetAnalysis = await desktop.analyzeSteelDataset({
          datasetId: dataset.id,
          correlationColumns,
          outlierIqrMultiplier: 1.5,
        });
        if (!cancelled) setCharts(buildReportCharts(analysis, dataset.sourceName));
      } catch (reason) {
        if (!cancelled) {
          setCharts([]);
          setError(errorMessage(reason, "无法生成报告图表"));
        }
      }
    })();
    return () => { cancelled = true; };
  }, [datasetId, datasets]);

  const generate = async () => {
    if (!topic.trim()) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const dataset = datasets.find((item) => item.id === datasetId);
      const dataHint = dataset
        ? `本次报告附带数据集「${dataset.sourceName}」（${dataset.rowCount} 行）的字段统计与相关性图表，请在正文中结合这些数据展开分析。`
        : "未指定数据集，请以检索到的知识库与文献证据为准。";
      const response = await desktop.desktopAgentChat({
        message: `请生成一份${type}，主题为“${topic}”。${dataHint}包含研究背景、数据与方法、结果、讨论、结论与局限。对每一条关键结论给出可追溯来源（作者 / 文档名 / 年份 / 来源位置），没有证据支持的地方明确标注“未记录”，不要编造。使用 Markdown，明确区分证据与推断，不展示思考过程。`,
        smartSearchEnabled: true,
      });
      setContent(response.answer);
    } catch (reason) {
      setError(errorMessage(reason, "报告生成失败"));
    } finally {
      setBusy(false);
    }
  };

  const rasterize = async (): Promise<ReportChartImage[]> =>
    Promise.all(charts.map(async (chart) => ({
      title: chart.title,
      png: await svgToPng(chart.svg, chart.width, chart.height),
      width: chart.width,
      height: chart.height,
    })));

  const exportWord = async () => {
    if (!content) return;
    setExporting("docx"); setError("");
    try {
      const blob = await buildReportDocx(content, topic || type, await rasterize());
      const saved = await saveReportBlob(blob, topic || type, "docx", "Word 文档");
      setNotice(saved ? "Word 文档已导出" : "已取消保存");
    } catch (reason) { setError(errorMessage(reason, "Word 导出失败")); }
    finally { setExporting(""); }
  };

  const exportPpt = async () => {
    if (!content) return;
    setExporting("pptx"); setError("");
    try {
      const blob = await buildReportPptx(content, topic || type, await rasterize());
      const saved = await saveReportBlob(blob, topic || type, "pptx", "PPT 演示文稿");
      setNotice(saved ? "PPT 已导出" : "已取消保存");
    } catch (reason) { setError(errorMessage(reason, "PPT 导出失败")); }
    finally { setExporting(""); }
  };

  const exportPdf = () => {
    if (!content) return;
    setNotice("已调起系统打印：选择 “Microsoft Print to PDF” 即可保存 PDF 文件。");
    printReport(content, topic || type, charts);
  };

  const downloadMarkdown = async () => {
    if (!content) return;
    const path = await desktop.saveFileDialog({ defaultPath: `${topic.replace(/[\\/:*?"<>|]/g, "_")}.md`, filters: [{ name: "Markdown", extensions: ["md"] }] });
    if (!path) return;
    const blob = new Blob([content], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = path.split(/[\\/]/).pop() || "suna-report.md";
    anchor.click();
    URL.revokeObjectURL(url);
  };

  return (
    <section className="suna-reports-page">
      <header className="suna-reports-header">
        <div>
          <span className="suna-module-kicker">SUNA RESEARCH REPORTS</span>
          <h1><FileChartColumn size={24} />科研报告</h1>
          <p>把数据、文献和 Agent 分析整理成可追溯的研究文档。</p>
        </div>
        <Button variant="primary" onClick={() => void generate()} disabled={busy}>
          {busy ? <Loader2 className="suna-spin" size={16} /> : <Sparkles size={16} />}{busy ? "生成中..." : "生成报告"}
        </Button>
      </header>
      {error && <div className="suna-reports-alert">{error}</div>}
      {notice && <div className="suna-reports-notice">{notice}</div>}
      <div className="suna-reports-layout">
        <section className="suna-reports-config">
          <label>报告类型
            <Select value={type} onValueChange={(value) => setType(value)}>
              <SelectTrigger aria-label="报告类型"><SelectValue /></SelectTrigger>
              <SelectContent>{REPORT_TYPES.map((item) => <SelectItem value={item} key={item}>{item}</SelectItem>)}</SelectContent>
            </Select>
          </label>
          <label>研究主题<Textarea value={topic} onChange={(event) => setTopic(event.target.value)} rows={5} /></label>
          <label>数据来源（可选）
            <Select value={datasetId} onValueChange={setDatasetId}>
              <SelectTrigger aria-label="数据来源"><SelectValue placeholder="不使用数据集" /></SelectTrigger>
              <SelectContent>
                <SelectItem value="">不使用数据集</SelectItem>
                {datasets.map((item) => <SelectItem value={item.id} key={item.id}>{item.sourceName} · {item.rowCount} 行</SelectItem>)}
              </SelectContent>
            </Select>
          </label>
          <p>生成时会调用 Agent 检索当前知识中心和文献证据；选择数据集后会附带字段统计与相关性图表。</p>
          <section className="suna-reports-saved" aria-label="已保存的报告">
            <h3>已保存报告（{saved.length}）</h3>
            <SavedReportsPanel reports={saved} busy={saving} onOpen={openSaved} onDelete={(report) => void removeSaved(report)} />
          </section>
        </section>
        <section className="suna-reports-preview">
          <div className="suna-reports-preview-head">
            <strong>报告预览</strong>
            {content && <div className="suna-reports-export-actions">
              <Button variant="primary" onClick={() => void saveReport()} disabled={saving}>{saving ? <Loader2 className="suna-spin" size={15} /> : <Save size={15} />}保存报告</Button>
              <Button variant="ghost" onClick={() => void downloadMarkdown()}><Download size={15} />Markdown</Button>
              <Button variant="ghost" onClick={() => void exportWord()} disabled={exporting !== ""}>{exporting === "docx" ? <Loader2 className="suna-spin" size={15} /> : <FileText size={15} />}Word</Button>
              <Button variant="ghost" onClick={() => void exportPpt()} disabled={exporting !== ""}>{exporting === "pptx" ? <Loader2 className="suna-spin" size={15} /> : <Presentation size={15} />}PPT</Button>
              <Button variant="ghost" onClick={exportPdf}><Printer size={15} />PDF（打印）</Button>
            </div>}
          </div>
          {charts.length > 0 && (
            <section className="suna-reports-charts" aria-label="报告图表">
              <h3><BarChart3 size={16} />数据与图表</h3>
              {charts.map((chart) => (
                <figure key={chart.id} dangerouslySetInnerHTML={{ __html: chart.svg }} />
              ))}
            </section>
          )}
          {content
            ? <article>{content}</article>
            : <div className="suna-reports-empty"><FileChartColumn size={30} /><strong>尚未生成报告</strong><span>设置主题后开始生成。</span></div>}
        </section>
      </div>
    </section>
  );
}
