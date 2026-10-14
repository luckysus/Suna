import {useEffect,useRef,useState} from "react";
import TemplateEditor,{algorithms,newTemplate} from "./TemplateEditor";
import type {Dataset,Preview,Service,Template,TemplateSummary,WorkflowRun} from "./types";
const states:Record<string,string>={queued:"等待执行",running:"正在执行",waiting_external:"等待子任务",paused:"已暂停",completed:"已完成",failed:"失败",cancelled:"已取消",interrupted:"已中断"};
export default function WorkflowPanel({projectId,archived=false,service,onOpenPlan}:{projectId:string;archived?:boolean;service:Service;onOpenPlan?:()=>void}){
  const [datasets,setDatasets]=useState<Dataset[]>([]),[versions,setVersions]=useState<TemplateSummary[]>([]);
  const [datasetId,setDatasetId]=useState(""),[template,setTemplate]=useState<Template>(newTemplate),[savedRevision,setSavedRevision]=useState(0),[dirty,setDirty]=useState(true);
  const [preview,setPreview]=useState<Preview|null>(null),[confirmed,setConfirmed]=useState(false),[runs,setRuns]=useState<WorkflowRun[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState("");
  const epoch=useRef(0),loading=useRef(0),nonce=useRef<string|null>(null);
  const activeRuns=useRef(runs),runRequest=useRef(0);activeRuns.current=runs;
  const dataset=datasets.find(d=>d.id===datasetId)||null;
  const invalidate=()=>{loading.current++;setPreview(null);setConfirmed(false);nonce.current=null;};
  const edit=(next:Template)=>{invalidate();setTemplate(next);setDirty(true);};
  useEffect(()=>{
    const token=++epoch.current;loading.current++;setDatasetId("");setTemplate(newTemplate());setSavedRevision(0);setDirty(true);setPreview(null);setConfirmed(false);setRuns([]);setError("");setBusy(true);nonce.current=null;
    const valid=()=>token===epoch.current;let unlisten:(()=>void)|undefined;let timer:ReturnType<typeof setTimeout>|undefined;
    const refresh=()=>{const request=++runRequest.current;return service.listRuns(projectId).then(r=>{if(valid()&&request===runRequest.current)setRuns(r);}).catch(()=>{if(valid())setError("无法读取工作流状态，请刷新后重试。");});};
    void Promise.all([service.listDatasets(),service.listTemplates(),service.listRuns(projectId)]).then(([d,v,r])=>{if(valid()){setDatasets(d);setVersions(v);setRuns(r);}}).catch(()=>{if(valid())setError("无法读取科研工作流配置。");}).finally(()=>{if(valid())setBusy(false);});
    void service.listenSchedulerProgress(e=>{if(valid()&&activeRuns.current.some(r=>r.task.id===e.id||r.nodes.some(n=>n.id===e.id))){clearTimeout(timer);timer=setTimeout(()=>{if(valid())void refresh();},250);}}).then(fn=>{if(valid())unlisten=fn;else fn();}).catch(()=>{if(valid())setError("状态通知连接失败，可手动刷新任务。");});
    window.addEventListener("focus",refresh);
    return()=>{epoch.current++;loading.current++;runRequest.current++;clearTimeout(timer);window.removeEventListener("focus",refresh);unlisten?.();};
  },[projectId,service]);
  const perform=async(work:()=>Promise<void>)=>{const token=epoch.current;setBusy(true);setError("");try{await work();}catch{if(token===epoch.current)setError("操作未完成，请检查数据、模板版本和规则后重试。");}finally{if(token===epoch.current)setBusy(false);}};
  const load=async(value:string)=>{invalidate();const request=loading.current;const [id,rev]=value.split("@");const version=await service.getTemplate(id,Number(rev));if(request!==loading.current)return;setTemplate(version.template);setSavedRevision(version.template.revision);setDirty(false);};
  const save=async()=>{const request=loading.current;const token=epoch.current;const version=await service.saveTemplate(template,savedRevision);if(request!==loading.current)return;setTemplate(version.template);setSavedRevision(version.template.revision);setDirty(false);invalidate();const items=await service.listTemplates();if(token===epoch.current)setVersions(items);};
  const inspect=async()=>{if(!dataset)return;invalidate();const request=loading.current;const result=await service.dryRun(template,{projectId,datasetId:dataset.id,datasetSha256:dataset.sourceSha256});if(request!==loading.current)return;
    if(result.projectId!==projectId||result.datasetId!==dataset.id||result.datasetSha256!==dataset.sourceSha256||result.templateId!==template.id||result.templateRevision!==template.revision||result.validationStatus!=="repository_validated_read_only"||result.workerValidation?.validationStatus!=='actual_offline_diagnostic'||result.workerValidation.networkUsed!==false)throw new Error("workflow_preview_changed");setPreview(result);nonce.current=crypto.randomUUID();};
  const start=async()=>{if(!dataset||!preview||!confirmed||dirty||!nonce.current)return;const token=epoch.current;
    const run=await service.start({projectId,datasetId:dataset.id,datasetSha256:dataset.sourceSha256,templateId:template.id,templateRevision:template.revision,dryRunSha256:preview.dryRunSha256,nonce:nonce.current,confirmed:true});
    if(token!==epoch.current)return;if(run.projectId!==projectId)throw new Error("workflow_project_mismatch");runRequest.current++;setRuns(old=>[run,...old.filter(r=>r.task.id!==run.task.id)]);setConfirmed(false);};
  const taskAction=async(run:WorkflowRun,retry:boolean)=>{const token=epoch.current;const request=++runRequest.current;await(retry?service.retryBackgroundTask(run.task.id):service.cancelBackgroundTask(run.task.id));const updated=await service.getRun(projectId,run.task.id);if(token===epoch.current&&request===runRequest.current)setRuns(old=>old.map(r=>r.task.id===run.task.id?updated:r));};
  return <section aria-label="科研工作流模板"><h3>双性能建模与联合优化</h3><p>分别训练两个预测模型，联合优化后生成待人工确认的实验计划。</p>
    {error&&<p role="alert">{error}</p>}<fieldset disabled={busy||archived}><legend>已有模板与数据</legend>
      <label>已保存模板<select value={dirty?"":`${template.id}@${template.revision}`} onChange={e=>{if(e.target.value)void perform(()=>load(e.target.value));}}><option value="">选择模板版本</option>{versions.map(v=><option key={`${v.id}@${v.revision}`} value={`${v.id}@${v.revision}`}>{v.name} · v{v.revision}</option>)}</select></label>
      <button type="button" onClick={()=>{invalidate();setTemplate(newTemplate());setSavedRevision(0);setDirty(true);}}>新建流程模板</button>
      <label>工作流数据集<select value={datasetId} onChange={e=>{invalidate();setDatasetId(e.target.value);if(!savedRevision)edit(newTemplate());}}><option value="">选择已激活数据集</option>{datasets.map(d=><option key={d.id} value={d.id} disabled={d.mappingState!=="ready"}>{d.sourceName}{d.mappingState!=="ready"?"（需先激活）":""}</option>)}</select></label>
    </fieldset>
    <TemplateEditor template={template} dataset={dataset} disabled={busy||archived} onChange={edit}/>
    <button type="button" disabled={busy||archived||!dirty||!dataset} onClick={()=>void perform(save)}>保存独立模板版本</button>
    <button type="button" disabled={busy||archived||dirty||!dataset} onClick={()=>void perform(inspect)}>只读预览</button>
    {preview&&<div aria-label="已校验运行预览"><p>项目 {projectId} · 模板 v{preview.templateRevision} · 数据 SHA256 {preview.datasetSha256}</p><ol>{preview.validatedModels.map(m=><li key={m.name}>{m.name} · {algorithms[m.algorithm as keyof typeof algorithms]||m.algorithm} · 有效样本 {m.validRowCount} · 排除 {m.filteredRowCount}</li>)}<li>NSGA-II 联合优化 · {template.trials} 次评估</li><li>实验计划：待人工确认；不会写入测量数据</li></ol><p>运行上限 {preview.timeoutSeconds} 秒 · {preview.materialRules?.ruleCount||0} 条专业规则 · {preview.networkRequired?"需要外部服务":"本地计算"}</p>
      <p>计算依赖已实际检查：{preview.workerValidation.dependencies.map(d=>`${d.name} ${d.version||"版本未记录"}`).join(" · ")}</p><p>生成的模型版本保持未激活状态。</p><label><input type="checkbox" checked={confirmed} disabled={busy||archived} onChange={e=>setConfirmed(e.target.checked)}/>确认创建训练任务、模型版本、优化结果和待确认计划</label>
      <button type="button" disabled={busy||archived||!confirmed||dirty} onClick={()=>void perform(start)}>确认并开始工作流</button>
    </div>}
    <button type="button" disabled={busy} onClick={()=>void perform(async()=>{const token=epoch.current;const request=++runRequest.current;const result=await service.listRuns(projectId);if(token===epoch.current&&request===runRequest.current)setRuns(result);})}>刷新工作流状态</button>
    <ol aria-label="科研工作流运行">{runs.map(run=><li key={run.task.id}><strong>模板 v{run.templateRevision} · {states[run.task.state]} · {run.task.progress}%</strong><small>原任务 {run.task.id}</small>{run.task.error_code&&<p>运行未完成：{run.task.error_code}</p>}<ul>{run.nodes.map(n=><li key={n.id}>{n.kind==="compute_optimize_constrained"?"联合优化":"模型训练"} · {states[n.state]} · {n.progress}%</li>)}</ul>{run.task.can_cancel&&<button type="button" disabled={busy} onClick={()=>void perform(()=>taskAction(run,false))}>取消工作流</button>}{run.task.can_retry&&<button type="button" disabled={busy||archived} onClick={()=>void perform(()=>taskAction(run,true))}>重试原工作流</button>}{run.result&&<p>已生成待确认计划 {run.result.proposedPlanId} · 结果 SHA256 {run.result.contentSha256}{onOpenPlan&&<button type="button" onClick={onOpenPlan}>查看待确认实验计划</button>}</p>}</li>)}</ol>
  </section>;
}
