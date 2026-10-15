import {useEffect,useRef,useState} from 'react';
import type {ManagedInventory,ManagedPreview,ManagedService,ManagedSummary} from '../../../bridge/managedPackageTypes';

const messages:Record<string,string>={package_preview_changed:'资产或项目内容已变化，请重新读取清单并预览。',package_asset_hash_mismatch:'资产字节与冻结摘要不一致，请核对原始来源。',package_asset_unavailable:'所选资产已缺失、位于外部目录或超出支持范围。',package_asset_private_fields:'模型含有不适合打包的私有配置字段，未导出。',package_isolation_conflict:'已有隔离副本与包内容不一致，未覆盖。',package_isolation_incomplete:'已有隔离副本不完整，未覆盖。',package_asset_inventory_budget:'资产数量超过当前读取预算，请拆分项目。',package_export_write_failed:'保存或回读校验失败，请检查所选位置。'};
const roles={dataset_source:'数据集原文件',image_original:'原始显微图像',model_artifact:'模型产物'};
function valid(summary:ManagedSummary){return summary.schemaVersion==='research-managed-package.v1'&&summary.scope==='managed_assets_isolated'&&/^[a-f0-9]{64}$/.test(summary.packageSha256)&&summary.writesToLiveWorkspace===false&&summary.modelsActivated===false&&summary.measurementsCreated===false&&summary.rightsIndependentlyVerified===false&&summary.rightsStatus==='user_declared_authorized'&&summary.metadata.scope==='metadata_only'&&summary.assetCount===summary.assets.length&&new Set(summary.assets.map(a=>a.resourceId)).size===summary.assets.length&&summary.metadata.schemaVersion==='research-package-manifest.v1'&&summary.metadata.assetsIncluded===false;}

export default function ManagedPackagePanel({projectId,service}:{projectId:string;service:ManagedService}){
  const [inventory,setInventory]=useState<ManagedInventory|null>(null),[selected,setSelected]=useState<string[]>([]),[rights,setRights]=useState(false);
  const [preview,setPreview]=useState<ManagedPreview|null>(null),[inspection,setInspection]=useState<ManagedSummary|null>(null),[confirmed,setConfirmed]=useState(false);
  const [busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState('');
  const epoch=useRef(0),running=useRef(false);
  useEffect(()=>{epoch.current++;running.current=false;setInventory(null);setSelected([]);setRights(false);setPreview(null);setInspection(null);setConfirmed(false);setBusy(false);setError('');setNotice('');return()=>{epoch.current++;};},[projectId]);
  const invalidate=()=>{setPreview(null);setNotice('');};
  async function perform(action:(current:()=>boolean)=>Promise<void>){
    if(running.current)return;running.current=true;const version=epoch.current;const current=()=>version===epoch.current;
    setBusy(true);setError('');setNotice('');
    try{await action(current);}catch(e){if(current()){const code=typeof e==='string'?e:e instanceof Error?e.message:'';setError(Object.prototype.hasOwnProperty.call(messages,code)?messages[code]:'操作未完成，请核对资产、文件类型和权限后重试。');setConfirmed(false);setPreview(null);}}
    finally{if(current()){running.current=false;setBusy(false);}}
  }
  async function load(current:()=>boolean){
    setInventory(null);setSelected([]);setRights(false);setPreview(null);
    const result=await service.inventory(projectId);if(!current())return;
    if(result.sourceProjectId!==projectId||result.scope!=='managed_assets_inventory'||result.rawContentIncluded!==false)throw new Error('package_inventory_invalid');
    setInventory(result);
  }
  async function prepare(current:()=>boolean){
    if(!rights||!selected.length||!inventory)return;
    setPreview(null);const result=await service.preview({projectId,assetIds:selected,confirmedAssetRights:true});if(!current())return;
    if(!valid(result)||result.sourceProjectId!==projectId||result.assetCount!==selected.length||result.assets.some(a=>!selected.includes(a.resourceId)||a.sha256!==inventory.assets.find(v=>v.resourceId===a.resourceId)?.sha256))throw new Error('package_preview_changed');
    setPreview(result);
  }
  async function save(current:()=>boolean){
    if(!rights||!preview)return;
    const result=await service.export({projectId,assetIds:selected,confirmedAssetRights:true},preview.packageSha256);if(!current())return;
    if(result&&(!valid(result)||result.packageSha256!==preview.packageSha256||result.sourceProjectId!==projectId||result.writeVerified!==true))throw new Error('package_export_receipt_invalid');
    setPreview(null);setRights(false);setNotice(result?'受管科研资产包已保存并通过回读校验。':'导出已取消。');
  }
  async function inspect(current:()=>boolean){
    setInspection(null);setConfirmed(false);const result=await service.inspect();if(!current())return;
    if(!result){setNotice('文件选择已取消。');return;}if(!valid(result))throw new Error('package_inspection_invalid');setInspection(result);
  }
  async function importIsolated(current:()=>boolean){
    if(!inspection||!confirmed)return;const result=await service.importIsolated(inspection.packageSha256,true);if(!current())return;
    setConfirmed(false);if(!result){setNotice('导入已取消。');return;}
    if(!valid(result)||result.packageSha256!==inspection.packageSha256||result.sourceProjectId!==inspection.sourceProjectId||result.status!=='isolated_assets_stored'||result.assetsRestoredToIsolation!==true||result.assetsRestoredToLiveWorkspace!==false||result.readyForComputation!==false||result.reopenVerified!==true)throw new Error('package_isolation_receipt_invalid');
    setNotice(`已保存 ${result.assetCount} 项受管资产的隔离副本，重开与逐文件摘要校验通过；未导入当前项目。`);
  }
  return <details aria-label="受管科研资产包"><summary>受管科研资产包与隔离导入</summary>
    <p>显式选择本机受管数据、原始图像和模型。原始文件会保留自身内嵌元数据，请确认可分享。外部文件、系统配置及凭据不打包；导入副本不激活模型、不写实测，也不能直接开始计算。</p>
    {error&&<p role="alert">{error}</p>}{notice&&<p role="status">{notice}</p>}
    <fieldset disabled={busy}><legend>选择导出资产</legend><button type="button" onClick={()=>void perform(load)}>读取受管资产清单</button>
      {inventory&&<ul>{inventory.assets.map(a=><li key={a.resourceId}><label><input type="checkbox" aria-label={`选择资产 ${a.resourceId}`} disabled={a.availability!=='eligible_unverified'} checked={selected.includes(a.resourceId)} onChange={e=>{invalidate();setRights(false);setSelected(old=>e.target.checked?[...old,a.resourceId]:old.filter(id=>id!==a.resourceId));}}/>{a.name} · {roles[a.role]} · {a.bytes??'未知'} 字节 · {a.availability==='eligible_unverified'?'预览时核验摘要':a.availability==='unavailable_or_external'?'外部或缺失，未读取':'格式或大小暂不支持'}</label></li>)}</ul>}
      <label><input type="checkbox" checked={rights} disabled={!selected.length} onChange={e=>{invalidate();setRights(e.target.checked);}}/>确认所选资产及项目标题可分享，并具有打包权限</label>
      <button type="button" disabled={!rights||!selected.length} onClick={()=>void perform(prepare)}>预览所选受管资产</button>
      {preview&&<div aria-label="受管资产导出预览"><p>{preview.assetCount} 项资产 · {preview.assetBytes} 字节 · 外部依赖缺失 {preview.missingDependencies.length} 项</p><details><summary>实际文件摘要</summary>{preview.assets.map(a=><p key={a.resourceId} style={{overflowWrap:'anywhere'}}>{a.resourceId} · {a.sha256}</p>)}</details><button type="button" onClick={()=>void perform(save)}>选择位置并导出受管资产包</button></div>}
    </fieldset>
    <fieldset disabled={busy}><legend>检查与隔离导入</legend><button type="button" onClick={()=>void perform(inspect)}>选择受管资产包并检查</button>
      {inspection&&<div aria-label="受管资产检查结果"><p>来源项目 {inspection.sourceProjectId} · {inspection.assetCount} 项资产已核验；授权为提供者自声明。</p><label><input type="checkbox" checked={confirmed} onChange={e=>setConfirmed(e.target.checked)}/>确认仅保存到独立隔离副本，不覆盖当前项目</label><button type="button" disabled={!confirmed} onClick={()=>void perform(importIsolated)}>重新选择同一包并保存隔离副本</button></div>}
    </fieldset>
  </details>;
}
