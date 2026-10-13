import type { ComputeTrainingResult, OptimizationObjective, OptimizationVariable, ComputeOptimizationRequest } from "../../bridge/desktop";
import { Input } from "../../components/ui/input";
import { Button } from "../../components/ui/button";

type Constraint = ComputeOptimizationRequest["constraints"][number];
export function OptimizationObjectives({ objectives, setObjectives, models, variables, setVariables, constraints, setConstraints, availableVariables, additionalVariables, onAddVariable, onRemoveVariable }: {
  objectives: OptimizationObjective[]; setObjectives: (v: OptimizationObjective[]) => void;
  models: Record<string, ComputeTrainingResult>; variables: OptimizationVariable[]; setVariables: (v: OptimizationVariable[]) => void;
  constraints: Constraint[]; setConstraints: (v: Constraint[]) => void;
  availableVariables: OptimizationVariable[]; additionalVariables: string[]; onAddVariable: (name: string) => void; onRemoveVariable: (name: string) => void;
}) {
  const update = (index: number, patch: Partial<OptimizationObjective>) => setObjectives(objectives.map((o, i) => i === index ? { ...o, ...patch } : o));
  return <>
    <fieldset><legend>独立性能目标</legend>{objectives.map((o, i) => <div key={i}>
      <label>目标名称<Input aria-label={`目标 ${i+1} 名称`} value={o.name} onChange={(e) => update(i, { name: e.target.value })} /></label>
      <label>来源<select aria-label={`目标 ${i+1} 来源`} value={o.source} onChange={(e) => update(i, { source: e.target.value as OptimizationObjective["source"], modelId: Object.keys(models)[0], coefficients: {} })}><option value="prediction_model">独立预测模型</option><option value="formula">线性成本函数</option></select></label>
      {o.source === "prediction_model" ? <label>模型<select aria-label={`目标 ${i+1} 模型`} value={o.modelId ?? ""} onChange={(e) => update(i, { modelId: e.target.value })}>{Object.entries(models).map(([id, m]) => <option key={id} value={id}>{m.artifact.model_type} · {id.slice(0,8)} · {m.artifact.feature_names.join(", ")}</option>)}</select></label> : <div><p>成本 = 截距 + Σ（单位成本 × 变量）</p><label>截距<Input type="number" value={o.intercept ?? 0} onChange={(e) => update(i, { intercept: Number(e.target.value) })} /></label>{variables.map((v) => <label key={v.name}>{v.name} 单位成本<Input type="number" value={o.coefficients?.[v.name] ?? 0} onChange={(e) => update(i, { coefficients: { ...o.coefficients, [v.name]: Number(e.target.value) } })} /></label>)}</div>}
      <label>方向<select aria-label={`目标 ${i+1} 方向`} value={o.direction} onChange={(e) => update(i, { direction: e.target.value as OptimizationObjective["direction"] })}><option value="maximize">最大化</option><option value="minimize">最小化</option></select></label>
      <label>单位<Input aria-label={`目标 ${i+1} 单位`} value={o.unit} onChange={(e) => update(i, { unit: e.target.value })} /></label>
      <Button variant="ghost" onClick={() => setObjectives(objectives.filter((_, j) => j !== i))}>删除目标</Button>
    </div>)}<Button variant="ghost" disabled={objectives.length >= 4} onClick={() => setObjectives([...objectives, { name: `性能 ${objectives.length+1}`, source: "prediction_model", modelId: Object.keys(models)[0], direction: "maximize", unit: "" }])}>添加性能目标</Button></fieldset>
    <fieldset><legend>工艺范围与固定变量</legend><label>添加设计变量（可不参与模型预测）<select aria-label="添加设计变量" value="" onChange={(e) => { if (e.target.value) onAddVariable(e.target.value); }}><option value="">选择数据集变量</option>{availableVariables.filter((v) => !variables.some((current) => current.name === v.name)).map((v) => <option key={v.name} value={v.name}>{v.name}</option>)}</select></label>{variables.map((v, i) => {
      const change = (patch: Partial<OptimizationVariable>) => setVariables(variables.map((item, j) => j === i ? { ...item, ...patch } : item));
      return <div key={v.name}><strong>{v.name}</strong>{additionalVariables.includes(v.name) && <Button variant="ghost" onClick={() => onRemoveVariable(v.name)}>删除附加变量 {v.name}</Button>}<label>下限<Input type="number" aria-label={`${v.name} 下限`} value={v.min} onChange={(e) => change({ min: Number(e.target.value) })} /></label><label>上限<Input type="number" aria-label={`${v.name} 上限`} value={v.max} onChange={(e) => change({ max: Number(e.target.value) })} /></label><label>固定值（留空可搜索）<Input type="number" aria-label={`${v.name} 固定值`} value={v.fixedValue ?? ""} onChange={(e) => change({ fixedValue: e.target.value === "" ? null : Number(e.target.value) })} /></label></div>;
    })}</fieldset>
    <fieldset><legend>线性配比约束</legend><p>Σ（系数 × 变量）等于或不小于阈值；上限约束可使用负系数与负阈值。</p>{constraints.map((c, i) => {
      const change = (patch: Partial<Constraint>) => setConstraints(constraints.map((item, j) => j === i ? { ...item, ...patch } : item));
      return <div key={i}><select aria-label={`约束 ${i+1} 类型`} value={c.kind} onChange={(e) => change({ kind: e.target.value as Constraint["kind"] })}><option value="equality">等于</option><option value="inequality">不小于</option></select>{variables.map((v) => <label key={v.name}>{v.name} 系数<Input type="number" value={(c.coefficients as Record<string,number>)[v.name] ?? 0} onChange={(e) => change({ coefficients: { ...c.coefficients, [v.name]: Number(e.target.value) } })} /></label>)}<label>阈值<Input type="number" value={c.value} onChange={(e) => change({ value: Number(e.target.value) })} /></label><label>容差<Input type="number" min="0" value={c.tolerance ?? 1e-6} onChange={(e) => change({ tolerance: Number(e.target.value) })} /></label><Button variant="ghost" onClick={() => setConstraints(constraints.filter((_, j) => j !== i))}>删除约束</Button></div>;
    })}<Button variant="ghost" onClick={() => setConstraints([...constraints, { kind: "equality", coefficients: {}, value: 100, tolerance: 1e-6 }])}>添加配比约束</Button></fieldset>
  </>;
}
