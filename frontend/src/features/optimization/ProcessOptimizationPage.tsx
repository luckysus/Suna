import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, CalendarPlus, CheckCircle2, Download, Gauge, Loader2, Play, RefreshCw, Upload } from "lucide-react";
import { desktop, type ComputeTrainingResult, type ComputeOptimizationRequest, type OptimizationObjective, type OptimizationVariable, type ComputeOptimizationResult, type SteelDatasetRecord } from "../../bridge/desktop";
import { useResearchStore } from "../../stores/researchStore";
import MaterialConstraintsPanel from "./MaterialConstraintsPanel";
import { OptimizationObjectives } from "./OptimizationObjectives";
import { useComputeTask, ComputeTaskStatus } from "../tasks/useComputeTask";
import { Input } from "../../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../components/ui/select";
import OptimizationParetoChart from "./OptimizationParetoChart";
import { OptimizationHistoryPanel } from "./OptimizationHistoryPanel";
import { FileDropZone } from "../../components/common/FileDropZone";
import "./optimization.css";
import { errorMessage } from "../../utils/errorMessage";
import { Button } from "../../components/ui/button";

/** 第 38–43 章：工艺优化（算法可选、训练任务联动、Pareto 前沿、候选入库）。 */

type State = "idle" | "loading" | "running" | "error";
const numeric = (dataset: SteelDatasetRecord) => dataset.columns.filter((column) => column.inferredType === "number" && !column.duplicate);

const ALGORITHMS = [
  { value: "nsga2", label: "NSGA-II（多目标）", multiOnly: false },
  { value: "tpe", label: "贝叶斯优化 TPE（单目标）", multiOnly: true },
  { value: "grid", label: "网格搜索", multiOnly: false },
] as const;

export default function ProcessOptimizationPage() {
  const project=useResearchStore(s=>s.currentProject);
  const [datasets, setDatasets] = useState<SteelDatasetRecord[]>([]);
  const [datasetId, setDatasetId] = useState("");
  const [models, setModels] = useState<Record<string, ComputeTrainingResult>>({});
  const [objectives, setObjectives] = useState<OptimizationObjective[]>([]);
  const [additionalVariables, setAdditionalVariables] = useState<string[]>([]);
  const [variables, setVariables] = useState<OptimizationVariable[]>([]);
  const [constraints, setConstraints] = useState<ComputeOptimizationRequest["constraints"]>([]);
  const [algorithm, setAlgorithm] = useState<string>("");
  const [trials, setTrials] = useState(80);
  const [state, setState] = useState<State>("idle");
  const [result, setResult] = useState<ComputeOptimizationResult | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const dataset = datasets.find((item) => item.id === datasetId);
  const columns = useMemo(() => dataset ? numeric(dataset) : [], [dataset]);
  const multiObjective = objectives.length > 1;
  const computeTask = useComputeTask(`suna.optimization.${datasetId}`, desktop.getComputeOptimizationResult);
  useEffect(()=>{const id=new URLSearchParams(window.location.search).get("task");if(id)void computeTask.run(id).catch(e=>setError(String(e)));},[]);
  useEffect(() => { if (computeTask.result) { setResult(computeTask.result); setState("idle"); setNotice("优化完成，候选方案已更新"); } }, [computeTask.result]);

  const load = async () => {
    setState("loading");
    try {
      const items = await desktop.listSteelDatasets();
      setDatasets(items.filter((item) => item.mappingState === "ready"));
      if (!datasetId) setDatasetId(items.find((item) => item.mappingState === "ready")?.id ?? "");
    } catch (reason) {
      setError(errorMessage(reason, "无法加载数据集"));
    } finally {
      setState("idle");
    }
  };
  useEffect(() => { void load(); }, []);

  // 第 38 章体验：从已完成的训练任务里选模型，不再手工填写任务 ID。
  useEffect(() => {
    setObjectives([]); setModels({}); setVariables([]); setAdditionalVariables([]); setConstraints([]); setResult(null);
    if (!datasetId) return;
    let cancelled = false;
    void (async () => {
      try {
        const tasks = await desktop.listBackgroundTasks();
        if (cancelled) return;
        const usable = tasks.filter((task) => task.kind.startsWith("compute_train") && task.state === "completed" && task.dataset_id === datasetId);
        const entries = await Promise.all(usable.map(async (task) => [task.id, await desktop.getComputeTrainingResult(task.id)] as const));
        if (cancelled) return;
        const available = Object.fromEntries(entries.filter((entry): entry is readonly [string, ComputeTrainingResult] => entry[1] !== null));
        setModels(available);
        const first = usable.find((t) => t.dataset_id === datasetId && available[t.id])?.id;
        if (first) setObjectives([{ name: "性能 1", source: "prediction_model", modelId: first, direction: "maximize", unit: "" }]);
      } catch (reason) {
        if (!cancelled) setError(errorMessage(reason, "无法加载训练任务"));
      }
    })();
    return () => { cancelled = true; };
  }, [datasetId]);

  useEffect(() => {
    const names = Array.from(new Set(objectives.flatMap((o) => o.source === "prediction_model" && o.modelId ? models[o.modelId]?.artifact.feature_names ?? [] : [])));
    const inputNames = Array.from(new Set([...names, ...additionalVariables]));
    setVariables((current) => inputNames.map((name) => {
      const prior = current.find((v) => v.name === name);
      if (prior) return prior;
      const column = columns.find((c) => (c.canonicalField || c.originalName) === name);
      const trained = Object.values(models).find((m) => m.artifact.feature_names.includes(name));
      const range = trained?.artifact.applicability_range[trained.artifact.feature_names.indexOf(name)];
      return { name, unit:column?.unit??undefined, fieldId:column?.canonicalField?.includes(".")?column.canonicalField:undefined, min: column?.min ?? range?.min ?? 0, max: column?.max ?? range?.max ?? 1, fixedValue: null };
    }));
    setConstraints((current) => current.map((c) => ({ ...c, coefficients: Object.fromEntries(Object.entries(c.coefficients).filter(([name]) => inputNames.includes(name))) })));
  }, [objectives, models, columns, additionalVariables]);

  const run = async () => {
    if (!dataset || objectives.length === 0 || variables.length === 0) {
      setError("请选择数据集、已完成的训练任务，并至少选择一个优化目标");
      return;
    }
    if (multiObjective && (algorithm === "ga" || algorithm === "pso")) {
      setError("遗传算法与粒子群只支持单目标，多目标请选择 NSGA-II 或网格搜索");
      return;
    }
    setState("running"); setError(""); setResult(null);
    try {
      const task = await desktop.optimizeSteelProcess({
        datasetId: dataset.id,
        objectives,
        variables,
        constraints,
        trials: Math.max(10, Math.min(500, trials)),
        seed: 42,
        algorithm: algorithm === "" ? undefined : (algorithm as "nsga2" | "tpe" | "ga" | "pso" | "grid"),
      });
      const completed = await computeTask.run(task.id);
      setResult(completed);
      setNotice(`优化完成：${completed.method} · ${completed.trials_completed} 次评估 · 前沿 ${completed.pareto_front.length} 个解`);
    } catch (reason) {
      setState("error");
      setError(errorMessage(reason, "工艺优化失败"));
    } finally {
      setState("idle");
    }
  };

  const importDataset = async () => {
    const path = await desktop.openFileDialog({ multiple: false, directory: false, filters: [{ name: "数据文件", extensions: ["csv", "xlsx", "xls", "json"] }] });
    if (typeof path !== "string") return;
    await importPaths([path]);
  };

  // 第 68 章：拖拽与按钮走同一条导入链路。
  const importPaths = async (paths: string[]) => {
    if (paths.length === 0) return;
    try {
      for (const path of paths) await desktop.saveSteelDataset({ sourcePath: path });
      setNotice(`已保存 ${paths.length} 个数据集，请在数据实验室完成列映射并激活`);
      await load();
    } catch (reason) {
      setError(errorMessage(reason, "数据集导入失败"));
    }
  };

  const exportResult = async () => {
    if (!result || !dataset) return;
    try {
      const saved = await desktop.saveExportFile(`${dataset.sourceName.replace(/\.[^.]+$/, "")}-优化方案.json`, Array.from(new TextEncoder().encode(JSON.stringify(result, null, 2))));
      if (saved) setNotice(`候选方案已导出：${saved.path}`);
    } catch (reason) { setError(errorMessage(reason, "导出失败")); }

  };

  const addToPlan = async (index: number) => {
    if (!result || !dataset) return;
    const recommendation = result.recommendations[index];
    const boundsByName = new Map(columns.map((column) => [column.canonicalField || column.originalName, column]));
    try {
      if(project && computeTask.task?.id){
        const artifact=await desktop.findResearchTaskArtifact(project.id,computeTask.task?.id);
        if(!artifact)throw new Error("当前任务未归入此项目，请在科研项目中关联原任务后重试");
        await desktop.createResearchCandidatePlan(project.id,artifact,index,`方案 ${index+1}（${dataset.sourceName}）`);
        setNotice("候选方案已加入当前项目实验计划，保留优化成果溯源");return;
      }
      await desktop.createExperimentPlan({
        source: "optimization",
        title: `方案 ${String.fromCharCode(65 + (index % 26))}（${dataset.sourceName}）`,
        objectiveNote: result.objectives.map((name, i) => `${(result.directions?.[i] ?? result.direction) === "minimize" ? "最小化" : "最大化"} ${name}`).join(" / "),
        variables: result.feature_names.map((name) => {
          const column = boundsByName.get(name);
          return {
            name,
            low: column?.min ?? undefined,
            high: column?.max ?? undefined,
            value: recommendation.values[name],
          };
        }),
        recommendation: { ...recommendation, objective_specs: result.objective_specs, method: result.method, seed: result.deterministic_seed },
      });
      setNotice("候选方案已加入实验计划，可在「实验助手」查看");
    } catch (reason) {
      setError(errorMessage(reason, "无法加入实验计划"));
    }
  };

  return (
    <section className="suna-optimization-page">
      <header className="suna-optimization-header">
        <div>
          <span className="suna-module-kicker">SUNA PROCESS OPTIMIZATION</span>
          <h1><Gauge size={24} />工艺优化</h1>
          <p>在数据和已训练模型约束下探索可行的工艺方案。</p>
        </div>
        <div className="suna-optimization-actions">
          <Button variant="ghost" onClick={() => void load()}><RefreshCw size={15} />刷新数据</Button>
          <Button variant="ghost" onClick={() => void importDataset()}><Upload size={15} />导入数据</Button>
        </div>
      </header>
      {error && <div className="suna-optimization-alert"><AlertTriangle size={16} />{error}</div>}
      {notice && <div className="suna-optimization-notice"><CheckCircle2 size={16} />{notice}</div>}
      <FileDropZone
        extensions={["csv", "xlsx", "xls", "json"]}
        onFiles={(paths) => importPaths(paths)}
        onRejected={(reason) => setError(reason)}
        label="松开即可导入数据集"
        hint="支持 CSV / Excel / JSON"
      >
      <div className="suna-optimization-layout">
        <section className="suna-optimization-config">
          <div className="suna-optimization-title"><Gauge size={17} /><div><h2>优化任务配置</h2><span>目标、变量和约束</span></div></div>
          <label>数据集<Select value={datasetId} onValueChange={setDatasetId}>
            <SelectTrigger aria-label="数据集"><SelectValue /></SelectTrigger>
            <SelectContent><SelectItem value="">选择已激活数据集</SelectItem>
              {datasets.map((item) => <SelectItem value={item.id} key={item.id}>{item.sourceName}</SelectItem>)}
            </SelectContent>
          </Select></label>
          <label>优化算法<Select value={algorithm} onValueChange={setAlgorithm}>
            <SelectTrigger aria-label="优化算法"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="">自动（多目标 NSGA-II / 单目标 TPE）</SelectItem>
              {ALGORITHMS.map((item) => (
                <SelectItem value={item.value} key={item.value} disabled={item.multiOnly && multiObjective}>
                  {item.label}{item.multiOnly && multiObjective ? "（仅单目标）" : ""}
                </SelectItem>
              ))}
            </SelectContent>
          </Select></label>
          <MaterialConstraintsPanel variables={variables} onChange={setVariables}/>
          <OptimizationObjectives objectives={objectives} setObjectives={setObjectives} models={models} variables={variables} setVariables={setVariables} constraints={constraints} setConstraints={setConstraints} availableVariables={columns.map((c) => ({ name: c.canonicalField || c.originalName, min: c.min ?? 0, max: c.max ?? 1 }))} additionalVariables={additionalVariables} onAddVariable={(name) => setAdditionalVariables((current) => [...current, name])} onRemoveVariable={(name) => { setAdditionalVariables((current) => current.filter((n) => n !== name)); setObjectives((current) => current.map((o) => o.source === "formula" ? { ...o, coefficients: Object.fromEntries(Object.entries(o.coefficients ?? {}).filter(([n]) => n !== name)) } : o)); }} />
          <ComputeTaskStatus {...computeTask} />
          <label>搜索次数<Input type="number" min="10" max="500" step="10" value={trials} onChange={(event) => setTrials(Number(event.target.value))} /></label>
          <Button variant="primary" className="suna-optimization-run" onClick={() => void run()} disabled={state === "running" || state === "loading" || !!(computeTask.task && ["queued","running","waiting_external"].includes(computeTask.task.state))}>
            {state === "running" ? <Loader2 className="suna-spin" size={16} /> : <Play size={16} />}{state === "running" ? "优化中..." : "开始优化"}
          </Button>
        </section>
        <section className="suna-optimization-result">
          <div className="suna-optimization-title"><Gauge size={17} /><div><h2>候选方案</h2><span>{result ? `${result.method} · ${result.trials_completed} 次评估 · 前沿 ${result.pareto_front.length} 解` : "运行任务后显示 Pareto 候选方案"}</span></div>
            {result && <Button variant="ghost" onClick={() => void exportResult()}><Download size={14} />导出 JSON</Button>}
          </div>
          {result && result.recommendations.length > 0 && (
            <OptimizationParetoChart front={result.pareto_front} recommendations={result.recommendations} objectives={result.objectives.map((name,i) => `${name}${result.objective_specs?.[i]?.unit ? ` (${result.objective_specs[i].unit})` : ""} ${(result.directions?.[i] ?? result.direction) === "minimize" ? "↓" : "↑"}`)} />
          )}
          {result ? (
            <div className="suna-optimization-table">
              <div className="suna-optimization-row suna-optimization-row-head"><strong>方案</strong><strong>预测值</strong><strong>目标值</strong><strong>工艺取值</strong><strong>操作</strong></div>
              {result.recommendations.map((recommendation, index) => (
                <div className="suna-optimization-row" key={index}>
                  <span>方案 {String.fromCharCode(65 + (index % 26))}</span>
                  <span>{recommendation.prediction.toFixed(3)}</span>
                  {recommendation.material_assessment&&<span>硬约束通过 · 软惩罚 {recommendation.material_assessment.softPenalty} · {!recommendation.material_assessment.supportAssessed?"未配置支持域":recommendation.material_assessment.supportRisk?"支持域外推风险":"通过已配置支持域"}</span>}
                  <span>{recommendation.objectives.map((value) => value.toFixed(3)).join(" / ")}</span>
                  <span className="suna-optimization-values">{result.feature_names.map((name) => `${name}=${recommendation.values[name]?.toFixed(2) ?? "-"}`).join("，")}</span>
                  <span className="suna-optimization-actions-cell"><Button variant="ghost" onClick={() => void addToPlan(index)}><CalendarPlus size={13} />加入实验计划</Button></span>
                </div>
              ))}
            </div>
          ) : (
            <div className="suna-optimization-empty"><Gauge size={30} /><strong>尚未生成方案</strong><span>选择训练模型后运行优化。</span></div>
          )}
        </section>
      </div>
      <OptimizationHistoryPanel />
      </FileDropZone>
    </section>
  );
}
