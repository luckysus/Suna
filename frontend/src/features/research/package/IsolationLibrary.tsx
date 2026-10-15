import {useEffect,useRef,useState} from 'react';
import type {IsolationLibraryService,IsolationList,IsolationView} from '../../../bridge/isolationLibraryTypes';

const digest=(v:string)=>/^[a-f0-9]{64}$/.test(v);
const identity=(v:string)=>/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i.test(v);
const states={stored_unverified:'已保存，尚未重新校验',requires_original_archive:'旧副本需要原包校验',incomplete_or_damaged:'副本不完整或不可读取'};
function validPage(page:{offset:number;total:number;hasMore:boolean},offset:number,count:number){
  return page.offset===offset&&Number.isInteger(page.total)&&page.total>=0&&page.total<=10000&&count<=25&&count===Math.min(25,Math.max(0,page.total-offset))&&page.hasMore===(offset+count<page.total);
}
function validList(page:IsolationList,offset:number){
  return page.schemaVersion==='research-managed-isolations.v1'&&page.readOnly===true&&page.writesToLiveWorkspace===false&&validPage(page,offset,page.items.length)&&new Set(page.items.map(i=>i.isolationId)).size===page.items.length&&page.items.every(i=>digest(i.isolationId)&&Object.prototype.hasOwnProperty.call(states,i.state));
}
function validView(view:IsolationView,id:string,offset:number){
  const s=view.isolation;
  return view.schemaVersion==='research-managed-isolation-view.v1'&&view.readOnly===true&&s.schemaVersion==='research-managed-package.v1'&&s.scope==='managed_assets_isolated'&&s.rightsStatus==='user_declared_authorized'&&s.rightsIndependentlyVerified===false&&s.assetCount>0&&s.assets.length===s.assetCount&&digest(id)&&s.packageSha256===id&&s.isolationId===id&&s.status==='isolated_assets_stored'&&s.reopenVerified===true&&s.originalArchiveStored===true&&s.assetsRestoredToIsolation===true&&s.assetsRestoredToLiveWorkspace===false&&s.writesToLiveWorkspace===false&&s.modelsActivated===false&&s.measurementsCreated===false&&s.readyForComputation===false&&s.metadata.schemaVersion==='research-package-manifest.v1'&&s.metadata.sourceProjectId===s.sourceProjectId&&s.metadata.scope==='metadata_only'&&s.metadata.assetsIncluded===false&&view.total===s.metadata.metadataRecordCount&&validPage(view,offset,view.records.length)&&new Set(view.records.map(r=>r.originalId)).size===view.records.length&&view.records.every(r=>identity(r.originalId)&&identity(r.importedId)&&s.idMapping[r.originalId]===r.importedId&&r.record.id===r.originalId&&digest(r.recordSha256));
}

export default function IsolationLibrary({service}:{service:IsolationLibraryService}){
  const [page,setPage]=useState<IsolationList|null>(null),[view,setView]=useState<IsolationView|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('');
  const epoch=useRef(0),running=useRef(false);
  useEffect(()=>()=>{epoch.current++;},[]);
  async function perform(action:(current:()=>boolean)=>Promise<void>){
    if(running.current)return;running.current=true;const version=epoch.current;const current=()=>version===epoch.current;
    setBusy(true);setError('');
    try{await action(current);}catch(e){if(current()){setView(null);const code=typeof e==='string'?e:e instanceof Error?e.message:'';setError(code==='package_isolation_original_required'?'此旧副本未保存原包，请在上方重新选择原包并校验。':code==='package_isolation_inventory_budget'?'副本数量超过本次读取预算，未截断显示为完整结果。':'副本校验或读取未完成，请核对原包和隔离文件；未修改当前项目。');}}
    finally{if(current()){running.current=false;setBusy(false);}}
  }
  function load(offset:number){return perform(async current=>{
    setView(null);setPage(null);const result=await service.list(offset);if(!current())return;
    if(!validList(result,offset))throw new Error('invalid_isolation_list');setPage(result);
  });}
  function open(id:string,offset=0){return perform(async current=>{
    setView(null);const result=await service.open(id,offset);if(!current())return;
    if(!validView(result,id,offset))throw new Error('invalid_isolation_view');setView(result);
  });}
  return <details aria-label="已保存的隔离副本"><summary>查看已保存的隔离副本</summary>
    <p>列表只显示保存状态。打开时重新校验原包、独立数据库及资产；可查看原元数据和新ID映射。元数据引用不等于完整成果正文，副本不能直接计算。</p>
    {error&&<p role="alert">{error}</p>}
    <fieldset disabled={busy}><legend>本机隔离副本</legend><button type="button" onClick={()=>void load(0)}>读取隔离副本列表</button>
      {page&&<><p>共 {page.total} 个副本，本页 {page.items.length} 个；均需重新校验。</p><ol>{page.items.map(item=><li key={item.isolationId}>
        <code style={{overflowWrap:'anywhere'}}>{item.isolationId}</code><p>{states[item.state]}</p>
        <button type="button" disabled={item.state!=='stored_unverified'} onClick={()=>void open(item.isolationId)}>校验并打开 {item.isolationId.slice(0,8)}</button>
      </li>)}</ol><button type="button" disabled={!page.offset} onClick={()=>void load(Math.max(0,page.offset-25))}>前一页隔离副本</button><button type="button" disabled={!page.hasMore} onClick={()=>void load(page.offset+25)}>更多隔离副本</button></>}
      {view&&<section aria-label="已校验隔离元数据"><p role="status">重开校验通过：{view.isolation.assetCount} 项资产，{view.total} 条元数据引用。未导入当前项目。</p>
        <p>来源项目 {view.isolation.sourceProjectId} · 缺失依赖 {view.isolation.missingDependencies.length}</p>
        <ol>{view.records.map(r=><li key={r.originalId}><strong>{typeof r.record.name==='string'?r.record.name:String(r.record.kind)}</strong>
          <p style={{overflowWrap:'anywhere'}}>原ID {r.originalId} → 副本ID {r.importedId}</p><p style={{overflowWrap:'anywhere'}}>元数据记录摘要 {r.recordSha256}</p>
          {typeof r.record.source_artifact_sha256==='string'&&<p style={{overflowWrap:'anywhere'}}>原冻结成果摘要 {r.record.source_artifact_sha256}</p>}
          <details><summary>原始元数据记录</summary><pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{JSON.stringify(r.record,null,2)}</pre></details>
        </li>)}</ol><button type="button" disabled={!view.offset} onClick={()=>void open(view.isolation.isolationId,Math.max(0,view.offset-25))}>前一页元数据</button><button type="button" disabled={!view.hasMore} onClick={()=>void open(view.isolation.isolationId,view.offset+25)}>更多隔离元数据</button>
      </section>}
    </fieldset>
  </details>;
}
