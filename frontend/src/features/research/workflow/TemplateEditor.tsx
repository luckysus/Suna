import type {Algorithm, Column, Dataset, ModelNode, Template} from "./types";
import TrainingEvaluationConfig from "../../prediction/TrainingEvaluationConfig";
import LinearConstraintsEditor from "../../optimization/LinearConstraintsEditor";
export const algorithms: Record<Algorithm, string> = {linear_regression:"线性回归",elasticnet:"ElasticNet",random_forest:"Random Forest",hist_gradient_boosting:"HistGradientBoosting",lightgbm:"LightGBM",xgboost:"XGBoost",svr:"SVR",mlp:"MLP",transformer:"Transformer"};
function field(c:Column){return c.canonicalField || c.originalName;}
export function newTemplate():Template {
  return {schemaVersion:"research-workflow-template.v1",id:crypto.randomUUID(),revision:1,name:"双性能建模与联合优化",
    models:["linear_regression","random_forest"].map((a,i)=>({name:`性能 ${i+1}`,algorithm:a as Algorithm,targetColumn:-1,featureColumns:[],direction:"maximize",unit:null,splitPolicy:{kind:"random",validationFraction:0.2,seed:42}})),
    variables:[],constraints:[],trials:100,seed:42,timeoutSeconds:3600};
}
export default function TemplateEditor({template,dataset,disabled,onChange}:{template:Template;dataset:Dataset|null;disabled:boolean;onChange:(t:Template)=>void}){
  const numeric=dataset?.columns.filter(c=>c.inferredType==="number")||[];
  const updateModel=(index:number,patch:Partial<ModelNode>)=>{
    const next=template.models.map((m,i)=>i===index?{...m,...patch}:m);
    const targets=new Set(next.map(m=>m.targetColumn));
    const models=next.map(m=>{
      const auxiliary=m.splitPolicy.kind==="group"?m.splitPolicy.groupColumn:m.splitPolicy.kind==="time"?m.splitPolicy.timeColumn:undefined;
      return {...m,featureColumns:m.featureColumns.filter(v=>!targets.has(v)&&v!==auxiliary)};
    });
    const ordinals=new Set(models.flatMap(m=>m.featureColumns));
    const variables=numeric.filter(c=>ordinals.has(c.ordinal)).map(c=>template.variables.find(v=>v.name===field(c))||{
      name:field(c),...(c.unit?{unit:c.unit}:{}),min:c.min??NaN,max:c.max??NaN});
    onChange({...template,models,variables});
  };
  return <fieldset disabled={disabled}><legend>流程配置</legend>
    <label>模板名称<input value={template.name} maxLength={128} onChange={e=>onChange({...template,name:e.target.value})}/></label>
    {template.models.map((m,i)=><fieldset key={i}><legend>独立性能模型 {i+1}</legend>
      <label>性能名称 {i+1}<input value={m.name} onChange={e=>updateModel(i,{name:e.target.value})}/></label>
      <label>预测算法 {i+1}<select value={m.algorithm} onChange={e=>updateModel(i,{algorithm:e.target.value as Algorithm})}>{Object.entries(algorithms).map(([key,text])=><option key={key} value={key}>{text}</option>)}</select></label>
      <label>目标性能列 {i+1}<select value={m.targetColumn} onChange={e=>{const c=numeric.find(c=>c.ordinal===Number(e.target.value));if(c)updateModel(i,{targetColumn:c.ordinal,unit:c.unit,featureColumns:m.featureColumns.filter(v=>v!==c.ordinal)});}}><option value={-1}>选择性能列</option>{numeric.filter(c=>!template.models.some((other,n)=>n!==i&&other.targetColumn===c.ordinal)).map(c=><option key={c.ordinal} value={c.ordinal}>{field(c)}{c.unit?` (${c.unit})`:""}</option>)}</select></label>
      <label>优化方向 {i+1}<select value={m.direction} onChange={e=>updateModel(i,{direction:e.target.value as ModelNode["direction"]})}><option value="maximize">最大化</option><option value="minimize">最小化</option></select></label>
      <fieldset><legend>输入特征 {i+1}</legend>{numeric.filter(c=>!template.models.some(m=>m.targetColumn===c.ordinal)).map(c=><label key={c.ordinal}><input type="checkbox" checked={m.featureColumns.includes(c.ordinal)} disabled={c.ordinal===(m.splitPolicy.kind==="group"?m.splitPolicy.groupColumn:m.splitPolicy.kind==="time"?m.splitPolicy.timeColumn:undefined)} onChange={e=>updateModel(i,{featureColumns:e.target.checked?[...m.featureColumns,c.ordinal]:m.featureColumns.filter(v=>v!==c.ordinal)})}/>{field(c)}</label>)}</fieldset>
      <TrainingEvaluationConfig value={m.splitPolicy} onChange={splitPolicy=>updateModel(i,{splitPolicy})} columns={dataset?.columns.filter(c=>!template.models.some(m=>m.targetColumn===c.ordinal))||[]}/>
    </fieldset>)}
    <fieldset><legend>优化变量与工艺范围</legend>{template.variables.map((v,i)=><div key={v.name}><strong>{v.name}{v.unit?` (${v.unit})`:""}</strong>{(["min","max"] as const).map(key=><label key={key}>{v.name} {key==="min"?"下限":"上限"}<input type="number" step="any" value={Number.isFinite(v[key])?v[key]:""} onChange={e=>onChange({...template,variables:template.variables.map((a,n)=>n===i?{...a,[key]:e.target.valueAsNumber}:a)})}/></label>)}<label>{v.name} 专业字段<input value={v.fieldId||""} placeholder="已有约束使用的字段标识" onChange={e=>onChange({...template,variables:template.variables.map((a,n)=>n===i?{...a,fieldId:e.target.value||undefined}:a)})}/></label></div>)}{!template.variables.length&&<p>选择模型输入特征后配置范围。</p>}</fieldset>
    <fieldset><legend>固定变量（留空可搜索）</legend>{template.variables.map((v,i)=><label key={v.name}>{v.name}{v.unit?` (${v.unit})`:"（单位未记录）"}<input type="number" step="any" aria-label={`${v.name} 固定值`} min={v.min} max={v.max} value={v.fixedValue??""} onChange={e=>onChange({...template,variables:template.variables.map((item,j)=>j===i?{...item,fixedValue:e.target.value===""?null:e.target.valueAsNumber}:item)})}/></label>)}</fieldset>
    <LinearConstraintsEditor variables={template.variables} constraints={template.constraints} onChange={constraints=>onChange({...template,constraints})}/>
    <label>优化评估次数<input type="number" min={1} max={500} value={template.trials} onChange={e=>onChange({...template,trials:e.target.valueAsNumber})}/></label>
    <label>优化随机种子<input type="number" min={0} value={template.seed} onChange={e=>onChange({...template,seed:e.target.valueAsNumber})}/></label>
    <label>总运行时间上限（秒）<input type="number" min={30} max={86400} value={template.timeoutSeconds} onChange={e=>onChange({...template,timeoutSeconds:e.target.valueAsNumber})}/></label>
    <p>项目中已启用的专业规则将在执行前校验；规则修订需要重新预览和确认。</p>
  </fieldset>;
}
