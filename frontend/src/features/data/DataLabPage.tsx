import { useEffect, useState } from "react";
import { BarChart3, Database, FileSpreadsheet, Loader2, RefreshCw, Search, Upload } from "lucide-react";
import { desktop, type DatasetAnalysis, type DatasetSeries, type MultivariateResult, type SteelDatasetRecord } from "../../bridge/desktop";
import { Input } from "../../components/ui/input";
import { DataLabAnalysisPanel, DataLabVisualizePanel } from "./DataLabPanels";
import MicrostructurePanel from "./MicrostructurePanel";
import MaterialMappingPanel from "./MaterialMappingPanel";
import { DataLabTablePanel } from "./DataLabTablePanel";
import { FileDropZone } from "../../components/common/FileDropZone";
import "./data-lab.css";
import { errorMessage } from "../../utils/errorMessage";
import { Button } from "../../components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "../../components/ui/tabs";

type DataTab = "manage" | "analysis" | "visualize" | "table" | "microstructure";

const tabs: Array<{ id: DataTab; label: string }> = [
  { id: "manage", label: "数据管理" },
  { id: "analysis", label: "数据分析" },
  { id: "visualize", label: "数据可视化" },
  { id: "table", label: "数据表" },
  { id: "microstructure", label: "显微组织" },
];

const DATA_EXTENSIONS = ["csv", "xlsx", "xls", "json"];

export default function DataLabPage() {
  const [datasets, setDatasets] = useState<SteelDatasetRecord[]>([]);
  const [selected, setSelected] = useState("");
  const [analysis, setAnalysis] = useState<DatasetAnalysis | null>(null);
  const [tab, setTab] = useState<DataTab>("manage");
  const [distributionOrdinal, setDistributionOrdinal] = useState<number | null>(null);
  const [series, setSeries] = useState<DatasetSeries | null>(null);
  const [tableSeries, setTableSeries] = useState<DatasetSeries | null>(null);
  const [tableBusy, setTableBusy] = useState(false);
  const [multivariate, setMultivariate] = useState<MultivariateResult | null>(null);
  const [clusterCount, setClusterCount] = useState(3);
  const [visualizeBusy, setVisualizeBusy] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [query, setQuery] = useState("");

  const dataset = datasets.find((item) => item.id === selected);

  const load = async () => {
    setBusy(true);
    try {
      const items = await desktop.listSteelDatasets();
      setDatasets(items);
      if (!selected) setSelected(items[0]?.id ?? "");
    } catch (reason) {
      setMessage(errorMessage(reason, "无法加载数据集"));
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => { void load(); }, []);

  const importFile = async () => {
    const path = await desktop.openFileDialog({ multiple: false, directory: false, filters: [{ name: "数据文件", extensions: DATA_EXTENSIONS }] });
    if (typeof path !== "string") return;
    await importPaths([path]);
  };

  // 第 68 章：拖拽与按钮走同一条导入链路。
  const importPaths = async (paths: string[]) => {
    if (paths.length === 0) return;
    setBusy(true);
    try {
      for (const path of paths) await desktop.saveSteelDataset({ sourcePath: path });
      setMessage(paths.length === 1 ? "数据集已保存，请完成列映射后激活" : `已保存 ${paths.length} 个数据集，请完成列映射后激活`);
      await load();
    } catch (reason) {
      setMessage(errorMessage(reason, "导入失败"));
    } finally {
      setBusy(false);
    }
  };

  const analyze = async () => {
    if (!dataset) return;
    setBusy(true);
    try {
      // 相关性只在显式给出字段时才计算：这里传入全部数值列（最多 12 个，避免矩阵过大）。
      const correlationColumns = dataset.columns
        .filter((column) => column.inferredType === "number" && !column.duplicate)
        .map((column) => column.ordinal)
        .slice(0, 12);
      const result = await desktop.analyzeSteelDataset({
        datasetId: dataset.id,
        correlationColumns,
        outlierIqrMultiplier: 1.5,
      });
      setAnalysis(result);
      const numeric = result.columns.filter((column) => column.inferredType === "number" && column.distribution.length > 0);
      setDistributionOrdinal(numeric[0]?.ordinal ?? null);
    } catch (reason) {
      setMessage(errorMessage(reason, "数据分析失败"));
    } finally {
      setBusy(false);
    }
  };

  const openTab = (next: DataTab) => {
    setTab(next);
    if (next === "manage" || !dataset) return;
    if (next === "table") { if (!tableSeries) void loadTable(); return; }
    if (!analysis) void analyze();
    if (next === "visualize" && !series) void loadVisualization();
  };

  // 第 67 章：科研数据表格，读取全部非重复字段用于排序 / 筛选 / 分页 / 复制 / 导出。
  const loadTable = async () => {
    if (!dataset) return;
    const ordinals = dataset.columns.filter((column) => !column.duplicate).map((column) => column.ordinal);
    if (ordinals.length === 0) { setMessage("该数据集没有可展示的字段"); return; }
    setTableBusy(true);
    try {
      setTableSeries(await desktop.readSteelDatasetSeries({ datasetId: dataset.id, columns: ordinals }));
    } catch (reason) {
      setMessage(errorMessage(reason, "无法读取数据表"));
    } finally {
      setTableBusy(false);
    }
  };

  // 参与多元分析的数值列：最多 6 个，避免 PCA/聚类的维度与载荷难以解读。
  const numericOrdinals = (dataset?.columns ?? [])
    .filter((column) => column.inferredType === "number" && !column.duplicate)
    .map((column) => column.ordinal)
    .slice(0, 6);

  const loadVisualization = async (clusters = clusterCount) => {
    if (!dataset || numericOrdinals.length < 2) {
      setMessage("散点图与 PCA 至少需要两个数值字段");
      return;
    }
    setVisualizeBusy(true);
    try {
      const [seriesResult, multivariateResult] = await Promise.all([
        desktop.readSteelDatasetSeries({ datasetId: dataset.id, columns: numericOrdinals }),
        desktop.analyzeSteelDatasetMultivariate({
          datasetId: dataset.id,
          columns: numericOrdinals,
          components: 2,
          clusters,
        }),
      ]);
      setSeries(seriesResult);
      setMultivariate(multivariateResult);
    } catch (reason) {
      setMessage(errorMessage(reason, "无法加载可视化数据"));
    } finally {
      setVisualizeBusy(false);
    }
  };

  const numericColumns = analysis?.columns.filter((column) => column.inferredType === "number") ?? [];
  const distributionCandidates = numericColumns.filter((column) => column.distribution.length > 0);
  const distributionColumn = distributionCandidates.find((column) => column.ordinal === distributionOrdinal) ?? distributionCandidates[0] ?? null;
  const chartData = numericColumns.filter((column) => column.mean != null).slice(0, 8)
    .map((column) => ({ name: column.name, mean: Number(column.mean?.toFixed(3)) }));
  const filteredDatasets = datasets.filter((item) => item.sourceName.toLocaleLowerCase().includes(query.toLocaleLowerCase()));

  return (
    <section className="suna-data-page">
      <header className="suna-data-header">
        <div>
          <span className="suna-module-kicker">SUNA DATA LABORATORY</span>
          <h1><BarChart3 size={24} />数据实验室</h1>
          <p>管理实验数据，检查质量并为预测与优化准备可信数据。</p>
        </div>
        <div className="suna-data-actions">
          <Button variant="ghost" onClick={() => void load()}><RefreshCw size={15} />刷新</Button>
          <Button variant="primary" onClick={() => void importFile()}><Upload size={15} />导入数据</Button>
        </div>
      </header>

      <Tabs value={tab} onValueChange={(value) => openTab(value as DataTab)}>
        <TabsList className="suna-data-tabs">
          {tabs.map((item) => (
            <TabsTrigger
              key={item.id}
              value={item.id}
              className={tab === item.id ? "is-active" : ""}
            >
              {item.label}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>

      {message && <div className="suna-data-message">{message}</div>}

      {tab === "microstructure" ? <MicrostructurePanel/> : <FileDropZone
        extensions={DATA_EXTENSIONS}
        onFiles={(paths) => importPaths(paths)}
        onRejected={(reason) => setMessage(reason)}
        label="松开即可导入数据集"
        hint="支持 CSV / Excel / JSON"
      >
      <div className="suna-data-layout">
        <section className="suna-data-list">
          <div className="suna-data-title">
            <Database size={17} />
            <div><h2>数据文件</h2><span>{datasets.length} 个数据集</span></div>
          </div>
          <label className="suna-data-search">
            <Search size={14} />
            <Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索数据文件" />
          </label>
          <div className="suna-data-files">
            {filteredDatasets.map((item) => (
              <button
                className={`suna-data-item ${item.id === selected ? "is-active" : ""}`}
                key={item.id}
                onClick={() => { setSelected(item.id); setAnalysis(null); setTab("manage"); }}
              >
                <FileSpreadsheet size={17} />
                <span>
                  <strong>{item.sourceName}</strong>
                  <small>{item.rowCount} 行 · {item.columnCount} 列 · {item.format}</small>
                </span>
                <em>{item.mappingState === "ready" ? "已激活" : "待映射"}</em>
              </button>
            ))}
            {filteredDatasets.length === 0 && <div className="suna-data-empty">暂无数据集，请导入 CSV 或 Excel。</div>}
          </div>
        </section>

        <section className="suna-data-analysis">
          <div className="suna-data-analysis-head">
            <div className="suna-data-title">
              <BarChart3 size={17} />
              <div>
                <h2>{dataset ? dataset.sourceName : "实验数据"}</h2>
                <span>{dataset ? `${dataset.rowCount} 条记录 · ${dataset.columnCount} 个字段` : "选择数据文件查看分析"}</span>
              </div>
            </div>
            {dataset && (
              <Button variant="primary" onClick={() => void analyze()} disabled={busy}>
                {busy ? <Loader2 className="suna-spin" size={15} /> : <BarChart3 size={15} />}分析数据
              </Button>
            )}
          </div>

          {tab === "manage" && (
            <div className="suna-data-empty large">
              <Database size={30} />
              <strong>选择数据集后开始分析</strong>
              <span>「数据分析」查看质量与分布，「数据可视化」查看相关性、分布与对比图。</span>
            </div>
          )}

          {(tab === "analysis" || tab === "visualize") && !analysis && (
            <div className="suna-data-empty large">
              <BarChart3 size={30} />
              <strong>{busy ? "正在分析…" : "尚未生成分析结果"}</strong>
              <span>点击「分析数据」或重新选择数据集。</span>
            </div>
          )}

          {tab === "manage" && dataset && <MaterialMappingPanel dataset={dataset} onSaved={()=>void load()}/>}
          {tab === "table" && <DataLabTablePanel series={tableSeries} datasetName={dataset?.sourceName} busy={tableBusy} />}

          {tab === "analysis" && analysis && (
            <DataLabAnalysisPanel analysis={analysis} numericColumns={numericColumns} chartData={chartData} datasetId={selected} datasetColumns={dataset?.columns ?? []} onCleaned={(next) => { setMessage(next); void load(); }} />
          )}

          {tab === "visualize" && analysis && (
            <DataLabVisualizePanel
              analysis={analysis}
              numericColumns={numericColumns}
              distributionColumn={distributionColumn}
              distributionCandidates={distributionCandidates}
              onDistributionChange={setDistributionOrdinal}
              series={series}
              multivariate={multivariate}
              clusterCount={clusterCount}
              onClusterCountChange={setClusterCount}
              onClusterCommit={() => void loadVisualization()}
              busy={visualizeBusy}
            />
          )}
        </section>
      </div>
      </FileDropZone>}
    </section>
  );
}
