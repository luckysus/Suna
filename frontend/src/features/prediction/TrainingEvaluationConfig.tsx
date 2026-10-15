import type { TrainingSplitPolicy } from "../../bridge/desktop";
import { Input } from "../../components/ui/input";

type EvaluationColumn = {ordinal: number; originalName: string; duplicate?: boolean};

export default function TrainingEvaluationConfig({ value, onChange, columns }: { value: TrainingSplitPolicy; onChange: (v: TrainingSplitPolicy) => void; columns: readonly EvaluationColumn[] }) {
  const change = (patch: Partial<TrainingSplitPolicy>) => onChange({ ...value, ...patch });
  return <fieldset className="suna-scientific-config"><legend>科研训练评估</legend>
    <label>划分策略<select aria-label="划分策略" value={value.kind} onChange={(e) => { const kind = e.target.value as TrainingSplitPolicy["kind"]; change({ kind, groupColumn: kind === "group" ? value.groupColumn : undefined, timeColumn: kind === "time" ? value.timeColumn : undefined }); }}><option value="random">随机划分</option><option value="group">炉次 / 批次分组</option><option value="time">时间外推</option></select></label>
    {value.kind !== "random" && <label>{value.kind === "group" ? "分组列" : "时间列"}<select aria-label="分组或时间列" value={(value.kind === "group" ? value.groupColumn : value.timeColumn) ?? ""} onChange={(e) => change(value.kind === "group" ? { groupColumn: e.target.value === "" ? undefined : Number(e.target.value) } : { timeColumn: e.target.value === "" ? undefined : Number(e.target.value) })}><option value="">选择列（不作为模型输入）</option>{columns.filter((c) => !c.duplicate).map((c) => <option value={c.ordinal} key={c.ordinal}>{c.originalName}</option>)}</select></label>}
    <label>验证集比例（开发数据内）<Input aria-label="验证集比例" type="number" min="0.05" max="0.5" step="0.05" value={value.validationFraction} onChange={(e) => change({ validationFraction: Number(e.target.value) })} /></label>
    <label>独立测试集比例<Input aria-label="独立测试集比例" type="number" min="0" max="0.45" step="0.05" value={value.testFraction ?? 0} onChange={(e) => change({ testFraction: Number(e.target.value) })} /></label>
    <label>Conformal 校准集比例<Input aria-label="校准集比例" type="number" min="0" max="0.45" step="0.05" value={value.calibrationFraction ?? 0} onChange={(e) => change({ calibrationFraction: Number(e.target.value) })} /></label>
    <label>交叉验证折数（0 关闭）<Input aria-label="交叉验证折数" type="number" min="0" max="10" value={value.folds ?? 0} onChange={(e) => change({ folds: Number(e.target.value) })} /></label>
    <label>超参数搜索次数（0 关闭）<Input aria-label="超参数搜索次数" type="number" min="0" max="12" value={value.searchTrials ?? 0} onChange={(e) => change({ searchTrials: Number(e.target.value) })} /></label>
    <label>随机种子<Input aria-label="随机种子" type="number" value={value.seed ?? 0} onChange={(e) => change({ seed: Number(e.target.value) })} /></label>
    <label><input type="checkbox" checked={value.learningCurve ?? false} onChange={(e) => change({ learningCurve: e.target.checked })} />计算学习曲线</label>
    <p>科研评估至少需要 10 个有效样本；搜索和学习曲线需要开启交叉验证。搜索支持 ElasticNet、SVR 和树模型，独立测试与校准比例之和须小于 70%。</p>
    <p>分组使用 Group K-Fold；时间使用前向验证。独立测试和校准数据不参与拟合或调参。95% 区间至少需要 19 个校准样本；分组或时间外推不保证可交换性，请结合测试覆盖率判断。</p>
  </fieldset>;
}
