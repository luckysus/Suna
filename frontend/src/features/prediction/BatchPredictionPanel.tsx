import { useEffect, useState } from "react";
import { desktop, type ComputeTrainingResult } from "../../bridge/desktop";
import { useComputeTask, ComputeTaskStatus } from "../tasks/useComputeTask";
import { Button } from "../../components/ui/button";
import { applicabilityLabel, applicabilityReason, applicabilityRow, parseBatchInput } from "./applicability";

export function parseBatchJson(text: string, names: string[]): number[][] {
  return parseBatchInput(text, names).rows;
}
export default function BatchPredictionPanel({ datasetId, model, onError, onNotice }: { datasetId: string; model: ComputeTrainingResult | null; onError: (s: string) => void; onNotice: (s: string) => void }) {
  const [models, setModels] = useState<Record<string, ComputeTrainingResult>>({});
  const [modelId, setModelId] = useState(model?.task_id ?? "");
  useEffect(() => {
    if (model) setModelId(model.task_id);
  }, [model]);
  useEffect(() => {
    if (!datasetId) return;
    let active = true;
    void (async () => {
      const tasks = (await desktop.listBackgroundTasks()).filter((t) => t.kind.startsWith("compute_train") && t.state === "completed" && t.dataset_id === datasetId);
      const entries = await Promise.all(tasks.map(async (t) => [t.id, await desktop.getComputeTrainingResult(t.id)] as const));
      if (!active) return;
      const available = Object.fromEntries(entries.filter((e): e is readonly [string, ComputeTrainingResult] => e[1] !== null));
      setModels(available);
      setModelId((id) => available[id] ? id : model?.task_id ?? Object.keys(available)[0] ?? "");
    })().catch((e) => { if (active) onError(String(e)); });
    return () => { active = false; };
  }, [datasetId]);
  const activeModel = models[modelId] ?? (model?.task_id === modelId ? model : null);
  const [text, setText] = useState("");
  const [rows, setRows] = useState<number[][]>([]);
  const task = useComputeTask(`suna.batch.${datasetId}`, desktop.getComputePredictionResult);
  useEffect(() => { if (task.result) onNotice(`批量预测完成：${task.result.predictions.length} 行`); }, [task.result]);
  const run = async () => {
    if (!activeModel) { onError("请先训练模型，再进行批量预测"); return; }
    try {
      const inputs = parseBatchInput(text, activeModel.artifact.feature_names);
      await desktop.setSetting(`suna.batch.inputs.${datasetId}`, JSON.stringify(inputs.rows));
      const submitted = await desktop.predictSteelModel({ datasetId, trainingTaskId: activeModel.task_id, featureRows: inputs.rows, inputSchema: inputs.inputSchema });
      setRows(inputs.rows);
      await task.run(submitted.id);
    } catch (e) { onError(e instanceof Error ? e.message : String(e)); }
  };
  useEffect(() => {
    let active = true;
    void desktop.getSetting(`suna.batch.inputs.${datasetId}`).then((value) => { if (active) { try { setRows(JSON.parse(value ?? "[]")); } catch { setRows([]); } } }).catch(() => { if (active) setRows([]); });
    return () => { active = false; };
  }, [datasetId]);
  const exportCsv = async () => {
    if (!task.result) return;
    try {
      const quote = (s: string) => `"${s.replace(/"/g, '""')}"`;
      const resultRows = task.result.input_rows ?? rows;
      const csv = [task.result.feature_names.map(quote).concat(["prediction","lower","upper","extrapolation","applicability_state","risk_codes","support_sha256"]).join(","), ...task.result.predictions.map((p, i) => {
        const risk = applicabilityRow(task.result?.applicability, i);
        return [...(resultRows[i] ?? []), p, task.result?.prediction_interval?.[i]?.lower ?? "", task.result?.prediction_interval?.[i]?.upper ?? "", risk.state === "outside_domain" ? "true" : risk.state === "unknown" ? "unknown" : "false", risk.state, quote(risk.reasons.map(reason => reason.code).join(";")), task.result?.applicability?.support_sha256 ?? ""].join(",");
      })].join("\r\n");
      const saved = await desktop.saveExportFile("批量预测.csv", Array.from(new TextEncoder().encode("\uFEFF"+csv)));
      if (saved) onNotice(`批量预测已导出：${saved.path}`);
    } catch (e) { onError(e instanceof Error ? e.message : String(e)); }
  };
  const busy = task.task && ["queued","running","waiting_external"].includes(task.task.state);
  return <section><h2>批量预测</h2><p>选择模型后粘贴或导入 JSON：每行可按特征名称提供对象，或按模型顺序提供数值数组。特征：{activeModel?.artifact.feature_names.join(", ") ?? "请先训练"}</p>
    {activeModel?.artifact.feature_schema?.fields && <p>模型训练单位：{activeModel.artifact.feature_schema.fields.map(field => `${field.name} (${field.unit ?? "单位未记录"})`).join("，")}</p>}
    <p>不同字段顺序或单位可使用 {`{"schema":[{"name":"temperature","unit":"K"}],"rows":[[1100]]}`} 格式，schema 必须包含全部模型特征。</p>
    <label>训练模型<select aria-label="批量预测模型" value={modelId} onChange={(e) => setModelId(e.target.value)}><option value="">选择已完成的模型</option>{Object.entries(models).map(([id,m]) => <option key={id} value={id}>{m.artifact.model_type} · {id.slice(0,8)}</option>)}</select></label>
    <input aria-label="导入批量 JSON" type="file" accept=".json,application/json" onChange={(e) => { const file=e.target.files?.[0]; if (file) void file.text().then(setText).catch((reason) => onError(String(reason))); }} />
    <textarea aria-label="批量预测输入" value={text} onChange={(e) => setText(e.target.value)} rows={8} />
    <Button onClick={() => void run()} disabled={!activeModel || !!busy}>提交整批预测</Button><ComputeTaskStatus {...task} />
    {task.result && <><Button onClick={() => void exportCsv()}>导出预测 CSV</Button><p>单变量训练范围筛查；联合支持域及物理可行性尚未评估。</p><table><thead><tr><th>行</th>{task.result.feature_names.map((n) => <th key={n}>{n}</th>)}<th>预测值</th><th>校准区间</th><th>适用域</th></tr></thead><tbody>{task.result.predictions.map((p,i) => {
      const risk = applicabilityRow(task.result?.applicability, i);
      return <tr key={i}><td>{i+1}</td>{(task.result?.input_rows ?? rows)[i]?.map((v,j) => <td key={j}>{v}</td>)}<td>{p}</td><td>{task.result?.prediction_interval?.[i] ? `${task.result.prediction_interval[i]?.lower} ~ ${task.result.prediction_interval[i]?.upper}` : "未校准"}</td><td>{applicabilityLabel(risk.state)}{risk.reasons.length > 0 && <details><summary>判定依据</summary>{risk.reasons.map((reason,j) => <div key={j}>{reason.feature ? `${reason.feature}：` : ""}{applicabilityReason(reason.code)}</div>)}</details>}</td></tr>;
    })}</tbody></table></>}
  </section>;
}
