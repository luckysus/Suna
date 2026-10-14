import { applicabilityLabel, applicabilityReason } from "../prediction/applicability";

import type { DomainPolicy, DomainFilter, ModelDomainEvidence } from "../../bridge/desktop";

export function OptimizationDomainPolicy({ value, onChange, disabled = false }: {
  value: DomainPolicy; onChange: (value: DomainPolicy) => void; disabled?: boolean;
}) {
  return <fieldset disabled={disabled}>
    <legend>模型训练支持域</legend>
    <label htmlFor="optimization-domain-policy">候选筛选策略</label>
    <select id="optimization-domain-policy" value={value} onChange={e => onChange(e.target.value as DomainPolicy)}>
      <option value="annotate_all">保留可行候选并标记逐模型风险</option>
      <option value="within_domain_only">仅保留通过全部模型训练范围筛查的候选</option>
    </select>
    <p>筛选仅作用于本次返回的 Pareto 前沿，旧模型缺少支持域时记为未知。训练范围筛查不能保证材料物理可行性或联合预测置信度。</p>
  </fieldset>;
}

export function OptimizationDomainSummary({ filter }: { filter?: DomainFilter }) {
  if (!filter) return <p>此历史任务尚未记录逐模型适用域。</p>;
  return <div role="status">
    <p>{filter.policy === "within_domain_only" ? "已按用户选择筛选" : "已保留全部返回候选"}：前沿 {filter.before} 个，保留 {filter.after} 个，排除 {filter.excluded} 个。</p>
    {filter.status === "insufficient_domain_candidates" && <p>训练支持域内候选不足；请复核范围、模型与数据。当前结果没有可加入实验计划的候选。</p>}
    <p>保持原专业约束。逐模型范围证据不构成已校准的联合置信度。</p>
  </div>;
}

export function CandidateDomainEvidence({ entries, objectives }: { entries?: ModelDomainEvidence[]; objectives: string[] }) {
  if (!entries) return <p>逐模型适用域未知（历史任务未记录）</p>;
  if (entries.length === 0) return <p>公式目标没有预测模型支持域证据</p>;
  return <ul aria-label="候选逐模型适用域">{entries.map(entry => <li key={entry.objective_index}>
    <strong>{objectives[entry.objective_index] ?? `目标 ${entry.objective_index + 1}`}：{applicabilityLabel(entry.risk.state)}</strong>
    {entry.risk.reasons.length > 0 && <span> · {entry.risk.reasons.map(reason => `${reason.feature ? `${reason.feature}：` : ""}${applicabilityReason(reason.code)}`).join("；")}</span>}
    <span> · 模型 {entry.model_id?.slice(0, 8) ?? "未知"} · 支持域 {entry.support_sha256?.slice(0, 8) ?? "未知"}</span>
    <span> · {entry.interval_status === "requires_exchangeability_assumption" ? "校准区间需要可交换性假设；外推覆盖率未验证" : "校准区间条件未知"}</span>
    <details><summary>查看该模型的支持域依据</summary>
      <p>模型产物 ID：{entry.model_id ?? "未记录"}</p>
      <p>模型 SHA：{entry.model_sha256 ?? "未记录"}</p>
      <p>训练数据 SHA：{entry.dataset_sha256 ?? "未记录"}</p>
      <p>训练支持域 SHA：{entry.support_sha256 ?? "未记录"}</p>
      {entry.risk.reasons.filter(reason => reason.threshold).map((reason,index) => <p key={index}>
        {reason.feature ?? "字段"}：{reason.threshold?.min ?? "未记录"} – {reason.threshold?.max ?? "未记录"} {reason.threshold?.unit ?? "单位未知"}
        {reason.threshold?.source === "training_split" ? "（来源：训练子集）" : "（阈值来源待复核）"}
      </p>)}
    </details>
  </li>)}</ul>;
}
