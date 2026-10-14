import { useEffect, useState } from "react";
import type { ScientificCardsResponse } from "../../bridge/desktop";

const show = (value: string | number | null | undefined) => value == null ? "未知" : String(value);
const percentage = (value: number | null) => value == null ? "未知" : `${(100 * value).toFixed(1)}%`;
const splitNames = {random:"随机划分",group:"分组划分",time:"时间划分"};

export default function ScientificModelCards({modelId,modelSha,loadCards}:{modelId:string;modelSha:string;loadCards:(id:string)=>Promise<ScientificCardsResponse>}) {
  const [result,setResult]=useState<ScientificCardsResponse|null>(null);
  const [error,setError]=useState("");
  useEffect(()=>{
    let active=true;
    setResult(null);setError("");
    loadCards(modelId).then(response=>{
      if (!active) return;
      if (response.cards.model_card.registry_id !== modelId || response.cards.model_card.model_sha256 !== modelSha.toLowerCase()) {
        setError("科研卡与当前模型版本不匹配");return;
      }
      setResult(response);
    }).catch(()=>{if(active)setError("无法读取经过校验的科研卡，请检查模型完整性或兼容性");});
    return ()=>{active=false;};
  },[modelId,modelSha,loadCards]);
  if(error)return <p role="alert">{error}</p>;
  if(!result)return <p role="status">正在校验科研卡…</p>;
  const model=result.cards.model_card,data=result.cards.data_card;
  return <section aria-label="模型卡与数据卡">
    <h3>科研模型卡 · v{show(model.version)}</h3>
    <p>{result.persisted ? "已随注册版本保存" : "旧模型只读生成，未改写原记录"} · 未审核</p>
    <dl className="suna-registry-facts">
      <div><dt>算法</dt><dd>{show(model.algorithm)}</dd></div>
      <div><dt>目标</dt><dd>{show(model.target.name)} · {model.target.unit ?? "单位未知"}</dd></div>
      <div><dt>模型 SHA</dt><dd>{model.model_sha256}</dd></div>
      <div><dt>数据 SHA</dt><dd>{show(model.dataset_sha256)}</dd></div>
      <div><dt>划分策略</dt><dd>{model.split_kind ? splitNames[model.split_kind] : "未知"}</dd></div>
      <div><dt>划分样本数</dt><dd>训练 {show(model.split_counts.train)} / 验证 {show(model.split_counts.validation)} / 测试 {show(model.split_counts.test)} / 校准 {show(model.split_counts.calibration)}</dd></div>
      <div><dt>测试 RMSE</dt><dd>{show(model.metrics.test.rmse)}</dd></div>
      <div><dt>测试样本数</dt><dd>{show(model.metrics.test.sample_count)}</dd></div>
      <div><dt>名义区间覆盖率</dt><dd>{percentage(model.interval.nominal_coverage)}</dd></div>
      <div><dt>测试集实际覆盖率</dt><dd>{percentage(model.interval.observed_test_coverage)}</dd></div>
    </dl>
    <p>区间依赖校准样本与未来样本可交换的假设；该假设尚未验证。适用域范围不等同材料物理可行性。</p>
    <h4>特征及单位</h4>
    <ul>{model.features.map((field,i)=><li key={i}>{show(field.name)} · {field.unit ?? "单位未知"}</li>)}</ul>
    <h4>已记录超参数</h4>
    {Object.keys(model.parameters).length ? <dl>{Object.entries(model.parameters).filter(([,value])=>typeof value==="number"||typeof value==="boolean").map(([name,value])=><div key={name}><dt>{name}</dt><dd>{String(value)}</dd></div>)}</dl> : <p>未知</p>}
    <h3>科研数据卡</h3>
    <dl className="suna-registry-facts">
      <div><dt>数据版本 ID</dt><dd>{show(data.dataset_id)}</dd></div>
      <div><dt>来源行数</dt><dd>{show(data.source_row_count)}</dd></div>
      <div><dt>有效行数</dt><dd>{show(data.valid_row_count)}</dd></div>
      <div><dt>排除行数</dt><dd>{show(data.filtered_row_count)}</dd></div>
      <div><dt>过滤规则</dt><dd>{data.filter_policy === "exclude_missing_target_reject_invalid_numbers" ? "排除缺失目标；拒绝无效数值" : "未知"}</dd></div>
      <div><dt>缺失值处理</dt><dd>{data.imputation === "train_mean" ? "仅用训练集均值填补" : "未知"}</dd></div>
      <div><dt>分组字段</dt><dd>{show(data.grouping_field)}</dd></div>
      <div><dt>时间字段</dt><dd>{show(data.time_field)}</dd></div>
      <div><dt>数据质量</dt><dd>未知，尚未审核</dd></div>
    </dl>
    <p>卡片摘要：{result.cards.cards_sha256}。仅提供来源元数据，不包含训练原始行。</p>
  </section>;
}
