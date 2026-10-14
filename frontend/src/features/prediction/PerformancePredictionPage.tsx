import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, BarChart3, CheckCircle2, Database, Loader2, Play, RefreshCw, Upload } from "lucide-react";
import { desktop, type ComputePredictionResult, type ComputeTrainingResult, type ShapExplanation, type SteelDatasetRecord, type SteelTrainingAlgorithm, type TrainingSplitPolicy } from "../../bridge/desktop";
import "./prediction.css";
import { FileDropZone } from "../../components/common/FileDropZone";
import { TRAINING_ALGORITHMS, algorithmLabel, columnDisplayName, featureImportanceShares, formatMetricValue } from "./predictionModel";
import ModelComparisonPanel from "./ModelComparisonPanel";
import ModelRegistryPanel from "./ModelRegistryPanel";
import CustomOnnxPanel from "./CustomOnnxPanel";
import PredictionPdpPanel from "./PredictionPdpPanel";
import { Checkbox } from "../../components/ui/checkbox";
import { Input } from "../../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../components/ui/select";
import ShapBarChart from "../../components/charts/ShapBarChart";
import { errorMessage } from "../../utils/errorMessage";
import { Button } from "../../components/ui/button";

import TrainingEvaluationConfig from "./TrainingEvaluationConfig";
import BatchPredictionPanel from "./BatchPredictionPanel";
import { buildShapleyPlan, explainShapleyPlan } from "./sampledShapley";
import { useComputeTask, ComputeTaskStatus, waitForComputeResult } from "../tasks/useComputeTask";
import { applicabilityLabel, applicabilityReason, applicabilityRow } from "./applicability";

type RunState = "idle" | "loading" | "running" | "error";
const numeric = (dataset: SteelDatasetRecord) => dataset.columns.filter((column) => column.inferredType === "number" && !column.duplicate);
const midpoint = (column: SteelDatasetRecord["columns"][number]) => column.min != null && column.max != null ? (column.min + column.max) / 2 : 0;

export default function PerformancePredictionPage() {
  const [datasets, setDatasets] = useState<SteelDatasetRecord[]>([]);
  const [datasetId, setDatasetId] = useState("");
  const [targetColumn, setTargetColumn] = useState("");
  const [featureColumns, setFeatureColumns] = useState<number[]>([]);
  const [algorithm, setAlgorithm] = useState<SteelTrainingAlgorithm>("linear_regression");
  const [splitPolicy, setSplitPolicy] = useState<TrainingSplitPolicy>({ kind: "random", validationFraction: 0.2, seed: 42, testFraction: 0.2, calibrationFraction: 0.2, folds: 0 });
  const [training, setTraining] = useState<RunState>("idle");
  const [prediction, setPrediction] = useState<RunState>("idle");
  const [trainingResult, setTrainingResult] = useState<ComputeTrainingResult | null>(null);
  const [predictionResult, setPredictionResult] = useState<ComputePredictionResult | null>(null);
  const [values, setValues] = useState<Record<number, string>>({});
  const [explanation, setExplanation] = useState<ShapExplanation | null>(null);
  const [explainBusy, setExplainBusy] = useState(false);
  const [explainError, setExplainError] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [sourceSnapshot,setSourceSnapshot]=useState<unknown>(null);
  const [tab, setTab] = useState<"single" | "batch" | "models" | "compare">("single");
  const trainingTask = useComputeTask(`suna.training.${datasetId}`, desktop.getComputeTrainingResult);
  const explanationTask = useComputeTask(`suna.explanation.${datasetId}`, desktop.getComputePredictionResult);
  const predictionTask = useComputeTask(`suna.prediction.${datasetId}`, desktop.getComputePredictionResult);
  useEffect(() => { if (trainingTask.result) { setTrainingResult(trainingTask.result); setTraining("idle"); } }, [trainingTask.result]);
  useEffect(() => { if (predictionTask.result) { setPredictionResult(predictionTask.result); setPrediction("idle"); } }, [predictionTask.result]);
  const dataset = datasets.find((item) => item.id === datasetId);
  const columns = useMemo(() => dataset ? numeric(dataset) : [], [dataset]);
  const metricCards = trainingResult ? ([["test", "独立测试集"], ["validation", "验证集"], ["train", "训练集"]] as const).flatMap(([split, label]) => {
    const metric = trainingResult.artifact.metrics[split];
    if (!metric) return [];
    return ([["R²", metric.r2], ["MAE", metric.mae], ["RMSE", metric.rmse], ["样本数", metric.sample_count]] as Array<[string, unknown]>)
      .map(([name, value]) => ({ key: `${split}-${name}`, label: `${label} ${name}`, value: formatMetricValue(value) }));
  }) : [];
  const importance = trainingResult ? featureImportanceShares(trainingResult.artifact.feature_names, trainingResult.artifact.feature_importance) : [];

  const load = async () => {
    setTraining("loading"); setError("");
    try {
      const items = await desktop.listSteelDatasets();
      setDatasets(items.filter((item) => item.mappingState === "ready"));
      if (!datasetId && items.some((item) => item.mappingState === "ready")) setDatasetId(items.find((item) => item.mappingState === "ready")!.id);
    } catch (reason) { setError(errorMessage(reason, "无法加载数据集")); }
    finally { setTraining("idle"); }
  };
  useEffect(() => { void load(); }, []);
  useEffect(() => {
    const id = new URLSearchParams(window.location.search).get('task');
    if (!id) return;
    let active = true;
    const controller=new AbortController();
    void desktop.getBackgroundTask(id).then(async task => {
      if (!active) return;
      if (task.kind.includes('train')) {
        const result = await waitForComputeResult(id,desktop.getComputeTrainingResult,controller.signal);
        if (active) {setSourceSnapshot({taskId:id,datasetId:task.dataset_id,modelType:result.artifact.model_type,featureNames:result.artifact.feature_names,metrics:result.artifact.metrics}); setNotice(`已打开原训练任务 ${id}`);}
      } else if (task.kind.includes('predict')) {
        const result = await waitForComputeResult(id,desktop.getComputePredictionResult,controller.signal);
        if (active) {setSourceSnapshot(result);setNotice(`已打开原预测任务 ${id}`);}
      } else throw new Error('该任务不是训练或预测任务');
    }).catch(reason => {if(active)setError(String(reason));});
    return () => {active=false;controller.abort();};
  }, []);

  useEffect(() => {
    if (!dataset) return;
    const available = numeric(dataset);
    const target = available.find((column) => column.canonicalField === "YS") ?? available[available.length - 1];
    setTargetColumn(target ? String(target.ordinal) : "");
    setFeatureColumns(available.filter((column) => column.ordinal !== target?.ordinal).slice(0, 8).map((column) => column.ordinal));
    setTrainingResult(null); setPredictionResult(null); setExplanation(null);
  }, [datasetId]);
  const target = Number(targetColumn);
  const featureRecords = trainingResult ? trainingResult.artifact.feature_names.map((name) => columns.find((column) => (column.canonicalField || column.originalName) === name)).filter((c): c is typeof columns[number] => !!c) : featureColumns.map((ordinal) => columns.find((c) => c.ordinal === ordinal)).filter((c): c is typeof columns[number] => !!c);
  useEffect(() => { setTrainingResult(null); setPredictionResult(null); setExplanation(null); }, [datasetId, targetColumn, featureColumns]);
  const toggleFeature = (ordinal: number) => setFeatureColumns((current) => current.includes(ordinal) ? current.filter((item) => item !== ordinal) : [...current, ordinal]);
  const train = async () => {
    if (!dataset || !targetColumn || featureColumns.length === 0) { setError("请选择数据集、预测目标和至少一个特征"); return; }
    setTraining("running"); setError(""); setPredictionResult(null);
    try {
      const task = await desktop.trainSteelDataset({ datasetId: dataset.id, targetColumn: target, featureColumns, algorithm, splitPolicy });
      const trained = await trainingTask.run(task.id);
      // 第 35 章模型中心：训练完成后自动注册为新版本；注册失败不阻断预测流程。
      try {
        const registered = await desktop.registerSteelModel({ taskId: trained.task_id });
        setNotice(`模型训练完成，已注册为 v${registered.version}，可在「模型管理」查看`);
      } catch {
        setNotice("模型训练完成，但自动注册失败，可在运行记录中手动注册");
      }
    } catch (reason) { setTraining("error"); setError(errorMessage(reason, "模型训练失败")); }
  };
  const runPrediction = async () => {
    if (!dataset || !trainingResult) return;
    if (featureRecords.some(column => values[column.ordinal] !== undefined && !values[column.ordinal].trim())) { setError("请输入全部变量，空值不会自动转换为零"); return; }
    const featureValues = featureRecords.map((column) => Number(values[column.ordinal] ?? midpoint(column)));
    if (featureValues.some((value) => !Number.isFinite(value))) { setError("输入变量必须是有效数字"); return; }
    setPrediction("running"); setError("");
    try {
      const task = await desktop.predictSteelModel({ datasetId: dataset.id, trainingTaskId: trainingResult.task_id, featureValues,
        inputSchema: featureRecords.map(column => ({ name: column.canonicalField || column.originalName, unit: trainingResult.artifact.feature_schema?.fields ? column.unit : undefined })) });
      const result = await predictionTask.run(task.id);
      setPredictionResult(result); setPrediction("idle");
    } catch (reason) { setPrediction("error"); setError(errorMessage(reason, "预测失败")); }
  };
  const explain = async () => {
    if (!trainingResult) return;
    // 特征顺序必须与训练时提交的顺序一致（featureColumns 的顺序）。
    const featureValues = featureRecords.map((column) => Number(values[column.ordinal] ?? midpoint(column)));
    if (featureValues.some((value) => !Number.isFinite(value))) { setExplainError("输入变量必须是有效数字"); return; }
    setExplainBusy(true); setExplainError("");
    try {
      if (trainingResult.artifact.model_type === "linear_regression") {
        setExplanation(await desktop.explainSteelModel({ modelId: trainingResult.artifact.model_id, features: featureValues }));
      } else {
        const baseline = trainingResult.artifact.preprocessing?.means;
        if (!baseline) throw new Error("模型缺少训练均值基线，请重新训练后解释");
        const plan = buildShapleyPlan(featureValues, baseline);
        const submitted = await desktop.predictSteelModel({ datasetId, trainingTaskId: trainingResult.task_id, featureRows: plan.rows });
        const predictions = await explanationTask.run(submitted.id);
        setExplanation(explainShapleyPlan(plan, predictions.predictions, trainingResult.artifact.feature_names, featureValues, trainingResult.artifact.model_type));
      }
    } catch (reason) {
      setExplanation(null);
      setExplainError(errorMessage(reason, "无法解释该模型"));
    } finally { setExplainBusy(false); }
  };
  const importDataset = async () => {
    const path = await desktop.openFileDialog({ multiple: false, directory: false, filters: [{ name: "数据文件", extensions: ["csv", "xlsx", "xls", "json"] }] });
    if (typeof path !== "string") return;
    await importDatasetPaths([path]);
  };
  const importDatasetPaths = async (paths: string[]) => {
    setError("");
    let saved = 0;
    for (const path of paths) {
      try { await desktop.saveSteelDataset({ sourcePath: path }); saved += 1; } catch (reason) { setError(errorMessage(reason, "数据集导入失败")); break; }
    }
    if (saved > 0) { setNotice(`已导入 ${saved} 个数据集，请在数据实验室完成列映射并激活`); await load(); }
  };
  return <FileDropZone extensions={["csv", "xlsx", "xls", "json"]} onFiles={(paths) => void importDatasetPaths(paths)} onRejected={(reason) => setNotice(reason)} label="松开即可导入数据集" hint="支持 CSV / Excel / JSON">
    <section className="suna-prediction-page">{sourceSnapshot!=null&&<details open><summary>原任务结果快照</summary><pre>{JSON.stringify(sourceSnapshot,null,2)}</pre></details>}<header className="suna-prediction-header"><div><span className="suna-module-kicker">SUNA PERFORMANCE PREDICTION</span><h1><BarChart3 size={24} />性能预测</h1><p>基于机器学习模型，快速预测钢铁材料的力学性能。</p></div><div className="suna-prediction-actions"><Button variant="ghost" onClick={importDataset}><Upload size={15} />导入数据</Button><Button size="icon" onClick={() => void load()} aria-label="刷新数据集"><RefreshCw size={16} /></Button></div></header><nav className="suna-prediction-tabs"><button className={tab === "single" ? "is-active" : ""} onClick={() => setTab("single")}>单点预测</button><button className={tab === "batch" ? "is-active" : ""} onClick={() => setTab("batch")}>批量预测</button><button className={tab === "models" ? "is-active" : ""} onClick={() => setTab("models")}>模型管理</button><button className={tab === "compare" ? "is-active" : ""} onClick={() => setTab("compare")}>模型比较</button></nav>{tab !== "single" && <div className="suna-prediction-notice">{tab === "batch" ? "批量输入按模型特征名称映射，可查看逐行结果并导出。" : tab === "compare" ? "模型比较会用同一组输入训练多个模型并对比预测值、误差与验证指标。" : "模型中心列出全部已注册模型版本，并支持自备 ONNX 模型预测。"}</div>}
    {error && <div className="suna-prediction-alert"><AlertTriangle size={16} />{error}</div>}{notice && <div className="suna-prediction-notice"><CheckCircle2 size={16} />{notice}</div>}
    {tab === "models" ? <div className="suna-prediction-modelcenter"><ModelRegistryPanel datasets={datasets} onNotice={setNotice} onError={setError} /><CustomOnnxPanel onNotice={setNotice} onError={setError} /></div> : tab === "batch" ? <BatchPredictionPanel datasetId={datasetId} model={trainingResult} onError={setError} onNotice={setNotice} /> : tab === "compare" ? <ModelComparisonPanel splitPolicy={splitPolicy} datasetId={datasetId} targetColumn={target} featureColumns={featureColumns} featureValues={featureColumns.map((ordinal) => { const column = columns.find((c) => c.ordinal === ordinal); return Number(values[ordinal] ?? (column ? midpoint(column) : 0)); })} onNotice={setNotice} onError={setError} /> : <div className="suna-prediction-layout"><section className="suna-prediction-config"><div className="suna-prediction-section-title"><Database size={17} /><div><h2>预测任务配置</h2><span>选择已激活的数据集和训练目标</span></div></div><label>数据集<Select value={datasetId} onValueChange={(value) => setDatasetId(value)}>
  <SelectTrigger aria-label="数据集"><SelectValue /></SelectTrigger>
  <SelectContent><SelectItem value="">选择数据集</SelectItem>{datasets.map((item) => <SelectItem value={item.id} key={item.id}>{item.sourceName} · {item.rowCount} 行</SelectItem>)}
  </SelectContent>
</Select></label><label>模型<Select value={algorithm} onValueChange={(value) => setAlgorithm(value as typeof algorithm)}>
  <SelectTrigger aria-label="模型"><SelectValue /></SelectTrigger>
  <SelectContent>{TRAINING_ALGORITHMS.map((item) => <SelectItem value={item.value} key={item.value}>{item.label} · {item.family}</SelectItem>)}
  </SelectContent>
</Select></label><label>预测目标<Select value={targetColumn} onValueChange={(value) => { setTargetColumn(value); setFeatureColumns((current) => current.filter((ordinal) => ordinal !== Number(value))); }}>
  <SelectTrigger aria-label="预测目标"><SelectValue /></SelectTrigger>
  <SelectContent><SelectItem value="">选择目标列</SelectItem>{columns.map((column) => <SelectItem value={String(column.ordinal)} key={column.ordinal}>{columnDisplayName(column)}</SelectItem>)}
  </SelectContent>
</Select></label><fieldset><legend>输入特征</legend>{columns.length === 0 && <span className="suna-prediction-muted">请先选择已激活的数据集</span>}{columns.map((column) => <label className="suna-prediction-check" key={column.ordinal}><Checkbox aria-label={column.originalName} checked={featureColumns.includes(column.ordinal)} disabled={column.ordinal === target || (splitPolicy.kind === "group" && column.ordinal === splitPolicy.groupColumn) || (splitPolicy.kind === "time" && column.ordinal === splitPolicy.timeColumn)} onCheckedChange={() => toggleFeature(column.ordinal)} />{columnDisplayName(column)}<small>{column.inferredType}</small></label>)}</fieldset><TrainingEvaluationConfig value={splitPolicy} onChange={(next) => { setSplitPolicy(next); const excluded = next.kind === "group" ? next.groupColumn : next.kind === "time" ? next.timeColumn : undefined; if (excluded !== undefined) setFeatureColumns((current) => current.filter((i) => i !== excluded)); }} columns={dataset?.columns ?? []} /><ComputeTaskStatus {...trainingTask} /><Button variant="primary" className="suna-prediction-train" onClick={() => void train()} disabled={training === "running" || training === "loading"}>{training === "running" ? <Loader2 className="suna-spin" size={16} /> : <Play size={16} />}{training === "running" ? "训练中..." : "训练模型"}</Button></section>
      <section className="suna-prediction-result"><div className="suna-prediction-section-title"><BarChart3 size={17} /><div><h2>预测结果</h2><span>{trainingResult ? `${algorithmLabel(trainingResult.artifact.model_type)} · 已完成` : "训练完成后显示结果"}</span></div></div>{trainingResult ? <><div className="suna-prediction-metrics">{metricCards.map((card) => <div key={card.key}><span>{card.label}</span><strong>{card.value}</strong></div>)}</div>{importance.length > 0 && <div className="suna-prediction-importance"><h3>特征重要性（Feature Importance）</h3>{importance.map((item) => <div key={item.name}><span>{item.name}</span><i><b style={{ width: `${Math.max(2, item.share * 100)}%` }} /></i><strong>{(item.share * 100).toFixed(1)}%</strong></div>)}</div>}<ComputeTaskStatus {...predictionTask} />{trainingResult.artifact.evaluation && <div><h3>科研评估</h3><p>交叉验证 {trainingResult.artifact.evaluation.folds.length} 折 · 独立测试外推样本 {trainingResult.artifact.evaluation.test_extrapolation_count} · 测试集区间覆盖率 {trainingResult.artifact.evaluation.test_interval_coverage == null ? "未校准或无测试集" : `${(100*trainingResult.artifact.evaluation.test_interval_coverage).toFixed(1)}%`}</p><p>{trainingResult.artifact.evaluation.interval_status}</p>{trainingResult.artifact.evaluation.learning_curve.map((p) => <p key={p.training_fraction}>训练数据 {Math.round(p.training_fraction*100)}% · 平均样本 {p.mean_train_count.toFixed(0)} · CV RMSE {p.mean_rmse.toFixed(3)}</p>)}</div>}<div className="suna-prediction-inputs"><h3>输入变量</h3>{featureRecords.map((column) => <label key={column.ordinal}>{columnDisplayName(column)}<Input type="number" value={values[column.ordinal] ?? ""} placeholder={`建议 ${midpoint(column).toFixed(3)}`} onChange={(event) => setValues((current) => ({ ...current, [column.ordinal]: event.target.value }))} /></label>)}<Button variant="primary" onClick={() => void runPrediction()} disabled={prediction === "running"}>{prediction === "running" ? <Loader2 className="suna-spin" size={15} /> : <Play size={15} />}{prediction === "running" ? "预测中..." : "运行预测"}</Button></div>{predictionResult && <div className="suna-prediction-output"><span>预测值</span><strong>{predictionResult.predictions.map((value) => Number(value).toFixed(3)).join(", ")}</strong>{predictionResult.prediction_interval?.[0] && <small>95% 预测区间 {predictionResult.prediction_interval[0].lower.toFixed(3)} ~ {predictionResult.prediction_interval[0].upper.toFixed(3)}（独立校准集 Split Conformal；需满足可交换性）</small>}{predictionResult.confidence != null && <small>置信度 {predictionResult.confidence.toFixed(3)}</small>}<div className="suna-prediction-warning">{applicabilityLabel(applicabilityRow(predictionResult.applicability, 0).state)}<small> · 单变量训练范围筛查；联合支持域及物理可行性尚未评估。</small>{applicabilityRow(predictionResult.applicability, 0).reasons.map((reason, index) => <div key={index}>{reason.feature ? `${reason.feature}：` : ""}{applicabilityReason(reason.code)}</div>)}</div></div>}<div className="suna-prediction-explain"><ComputeTaskStatus {...explanationTask} /><div className="suna-prediction-section-title"><BarChart3 size={15} /><div><h3>模型解释（Shapley 贡献）</h3><span>解释各输入变量对预测值的贡献</span></div><Button variant="ghost" onClick={() => void explain()} disabled={explainBusy}>{explainBusy ? <Loader2 className="suna-spin" size={14} /> : <Play size={14} />}解释本次输入</Button></div>{explainError && <div className="suna-prediction-warning"><AlertTriangle size={14} />{explainError}</div>}{explanation ? <><p>{explanation.method ? `${explanation.method === "baseline_shapley_exact" ? "枚举" : "采样"}基线 Shapley · 训练均值基线 · ${explanation.permutationCount} 个排列（非因果解释，掩码组合可能不满足材料约束）` : "线性模型精确 SHAP"}</p><ShapBarChart explanation={explanation} />{explanation.method === "baseline_shapley_sampled" && <p>贡献采样标准误：{explanation.values.map((v) => `${v.feature}=${v.standardError?.toFixed(4)}`).join("，")}</p>}</> : <p className="suna-prediction-muted">填写输入变量后点击「解释本次输入」。线性模型使用精确 SHAP；其他模型以训练均值为基线计算模型无关的置换 Shapley。最多 64 个特征。</p>}</div><PredictionPdpPanel datasetId={datasetId} trainingTaskId={trainingResult.task_id} featureNames={featureRecords.map((column) => column.originalName)} featureValues={featureRecords.map((column) => Number(values[column.ordinal] ?? midpoint(column)))} ranges={trainingResult.artifact.applicability_range} onError={setError} /></> : <div className="suna-prediction-empty"><BarChart3 size={30} /><strong>尚未生成预测</strong><span>训练模型后，这里会显示指标和预测结果。</span></div>}</section></div>}
  </section>;
  </FileDropZone>;
}
