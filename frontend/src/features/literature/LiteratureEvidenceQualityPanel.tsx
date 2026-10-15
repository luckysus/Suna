import {useEffect,useRef,useState} from 'react';
import type {EvidenceQualityPreview,EvidenceQualityService} from '../../bridge/evidenceQualityTypes';

const sourceLabels={matched:'索引引用匹配',missing:'索引来源缺失',changed:'索引来源已变化'};
const unitLabels={normalized:'单位可换算',unknown_field:'字段未识别',incompatible_or_overflow:'单位不兼容或换算溢出'};
const doiLabels={linked:'已关联本地原文',not_recorded:'DOI未记录',link_missing:'DOI原文关联缺失'};
const digest=(v:string)=>/^[a-f0-9]{64}$/.test(v);
function valid(preview:EvidenceQualityPreview,projectId:string){
  const r=preview.report;
  return preview.readOnly===true&&digest(preview.reportSha256)&&r.schemaVersion==='literature-evidence-quality.v1'&&r.algorithmVersion==='index-traceability.v1'&&r.projectId===projectId&&r.scope==='reviewed_numeric_evidence'&&r.sourceType==='literature'&&digest(r.registrySha256)&&r.reviewedEvidenceCount===r.checks.length&&r.checks.length<=1000&&r.checks.every(c=>digest(c.evidenceSha256)&&c.rawSourceBytesVerified===false&&c.comparability==='conditions_not_recorded'&&Object.prototype.hasOwnProperty.call(sourceLabels,c.indexedSourceState)&&Object.prototype.hasOwnProperty.call(unitLabels,c.unitState)&&Object.prototype.hasOwnProperty.call(doiLabels,c.doiState));
}

export default function LiteratureEvidenceQualityPanel({projectId,service}:{projectId:string|null;service:EvidenceQualityService}){
  const [preview,setPreview]=useState<EvidenceQualityPreview|null>(null),[offset,setOffset]=useState(0),[busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState('');
  const epoch=useRef(0),running=useRef(false);
  useEffect(()=>{epoch.current++;running.current=false;setPreview(null);setOffset(0);setBusy(false);setError('');setNotice('');return()=>{epoch.current++;};},[projectId]);
  async function perform(action:(current:()=>boolean)=>Promise<void>){
    if(running.current||!projectId)return;running.current=true;const version=epoch.current;const current=()=>version===epoch.current;
    setBusy(true);setError('');setNotice('');
    try{await action(current);}catch(e){if(current()){setPreview(null);const code=typeof e==='string'?e:e instanceof Error?e.message:'';setError(code==='literature_quality_preview_changed'?'证据、来源或字段配置已变化，请重新核查。':'核查未完成，请核对项目证据及来源；未改写原证据。');}}
    finally{if(current()){running.current=false;setBusy(false);}}
  }
  const r=preview?.report;
  return <details aria-label="文献证据溯源核查"><summary>文献证据溯源核查</summary>
    <p>仅核查当前项目已归档的数值证据、索引版本、引用和单位。未校验外部原文件、实验条件、测试方法或撤稿状态；本地关联不等于公网DOI验证，也不构成科研可信度评分。</p>
    {!projectId&&<p>请先选择科研项目。</p>}{error&&<p role="alert">{error}</p>}{notice&&<p role="status">{notice}</p>}
    <fieldset disabled={busy||!projectId}><legend>核对已审核证据</legend>
      <button type="button" onClick={()=>void perform(async current=>{setPreview(null);const result=await service.preview(projectId!);if(!current())return;if(!valid(result,projectId!))throw new Error('invalid_evidence_quality');setOffset(0);setPreview(result);})}>核查项目证据</button>
      {r&&<section aria-label="证据核查预览"><p>已归档 {r.reviewedEvidenceCount} 条：索引引用匹配 {r.counts.indexedReferenceMatched}，缺失 {r.counts.missingIndexedSource}，变化 {r.counts.changedIndexedSource}。</p>
        <p>DOI未记录 {r.counts.doiNotRecorded}，原文关联缺失 {r.counts.doiLinkMissing}，位置未记录 {r.counts.locationNotRecorded}，字段未识别 {r.counts.unknownField}，单位问题 {r.counts.incompatibleUnit}。</p>
        {!r.checks.length&&<p>尚无已归档证据；检索题录和本页AI草稿不计入已审核证据。</p>}
        <ol>{r.checks.slice(offset,offset+25).map(c=><li key={c.evidenceSha256}>
          <p>{c.field}：{c.value} {c.unit} · {sourceLabels[c.indexedSourceState]} · {doiLabels[c.doiState]} · {unitLabels[c.unitState]}</p>
          {c.unitState==='normalized'&&<p>规范单位值 {c.normalizedValue} {c.canonicalUnit}；实验条件未记录，不作直接性能比较。</p>}
          <p style={{overflowWrap:'anywhere'}}>版本 {c.versionId} · 片段 {c.chunkId} · 证据摘要 {c.evidenceSha256}</p>
        </li>)}</ol><button type="button" disabled={!offset} onClick={()=>setOffset(Math.max(0,offset-25))}>前一页核查记录</button><button type="button" disabled={offset+25>=r.checks.length} onClick={()=>setOffset(offset+25)}>更多核查记录</button>
        <button type="button" disabled={!r.checks.length} onClick={()=>void perform(async current=>{const result=await service.archive(projectId!,preview!.reportSha256);if(!current())return;if(result.reportSha256!==preview!.reportSha256||result.contentSha256!==preview!.reportSha256||!result.artifactId)throw new Error('invalid_evidence_quality_receipt');setNotice(`核查报告已保存为项目不可变成果 ${result.artifactId}。未写入训练数据或实测。`);})}>保存本次核查报告</button>
      </section>}
    </fieldset>
  </details>;
}
