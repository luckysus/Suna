import type {ComputeOptimizationRequest, OptimizationVariable} from "../../bridge/desktop";
import {Input} from "../../components/ui/input";
import {Button} from "../../components/ui/button";

type Constraint=ComputeOptimizationRequest["constraints"][number];

// Both editors submit the same named linear system to the original Rust/Worker path.
export default function LinearConstraintsEditor({variables,constraints,onChange}:{
  variables:Pick<OptimizationVariable,"name"|"unit">[];constraints:Constraint[];onChange:(v:Constraint[])=>void;
}){
  const names=new Set(variables.map(v=>v.name));
  const missing=[...new Set(constraints.flatMap(c=>Object.keys(c.coefficients).filter(n=>!names.has(n))))];
  return <fieldset><legend>线性配比约束</legend>
    <p>Σ（系数 × 变量）等于或不小于阈值；上限约束可使用负系数与负阈值。系数和阈值按各变量显示的单位填写，容差使用线性表达式的单位。</p>
    {missing.length>0&&<p role="status">约束引用了已移除的变量：{missing.join("、")}。请恢复变量或删除该约束后重新配置。</p>}
    {constraints.map((c,i)=>{
      const change=(patch:Partial<Constraint>)=>onChange(constraints.map((item,j)=>j===i?{...item,...patch}:item));
      return <fieldset key={i}><legend>约束 {i+1}</legend>
        <label>类型<select aria-label={`约束 ${i+1} 类型`} value={c.kind} onChange={e=>change({kind:e.target.value as Constraint["kind"]})}><option value="equality">等于</option><option value="inequality">不小于</option></select></label>
        {variables.map(v=><label key={v.name}>{v.name}{v.unit?` (${v.unit})`:"（单位未记录）"} 系数<Input type="number" step="any" aria-label={`约束 ${i+1} ${v.name} 系数`} value={Number.isFinite((c.coefficients as Record<string,number>)[v.name]??0)?(c.coefficients as Record<string,number>)[v.name]??0:""} onChange={e=>change({coefficients:{...c.coefficients,[v.name]:e.target.valueAsNumber}})}/></label>)}
        <label>阈值<Input type="number" step="any" aria-label={`约束 ${i+1} 阈值`} value={Number.isFinite(c.value)?c.value:""} onChange={e=>change({value:e.target.valueAsNumber})}/></label>
        <label>容差<Input type="number" step="any" min="0" aria-label={`约束 ${i+1} 容差`} value={Number.isFinite(c.tolerance??1e-6)?c.tolerance??1e-6:""} onChange={e=>change({tolerance:e.target.valueAsNumber})}/></label>
        <Button variant="ghost" aria-label={`删除约束 ${i+1}`} onClick={()=>onChange(constraints.filter((_,j)=>j!==i))}>删除约束</Button>
      </fieldset>;
    })}
    <Button variant="ghost" disabled={!variables.length||constraints.length>=64} onClick={()=>onChange([...constraints,{kind:"equality",coefficients:{},value:100,tolerance:1e-6}])}>添加配比约束</Button>
  </fieldset>;
}
