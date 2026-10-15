import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import ManagedPackagePanel from './ManagedPackagePanel';
import type {ManagedInventory,ManagedPreview,ManagedService,ManagedSummary} from '../../../bridge/managedPackageTypes';

const id='11111111-1111-4111-8111-111111111111';const sha='a'.repeat(64);
const inventory:ManagedInventory={sourceProjectId:'project-a',scope:'managed_assets_inventory',rawContentIncluded:false,rightsIndependentlyVerified:false,
  assets:[{resourceId:id,role:'dataset_source',name:'合成数据.csv',sha256:sha,bytes:20,availability:'eligible_unverified'},
    {resourceId:'22222222-2222-4222-8222-222222222222',role:'model_artifact',name:'外部模型',sha256:'b'.repeat(64),bytes:null,availability:'unavailable_or_external'}]};
const preview:ManagedPreview={schemaVersion:'research-managed-package.v1',scope:'managed_assets_isolated',packageSha256:'c'.repeat(64),sourceProjectId:'project-a',
  metadata:{schemaVersion:'research-package-manifest.v1',scope:'metadata_only',sourceProjectId:'project-a',receiptSha256:'d'.repeat(64),metadataRecordCount:2,edgeCount:0,missingDependencies:[],assetsIncluded:false,provenanceMode:'references_only',rightsStatus:'not_assessed',writesToLiveWorkspace:false,assetsRestored:false},
  assets:[{resourceId:id,role:'dataset_source',extension:'csv',sha256:sha,bytes:20}],assetCount:1,assetBytes:20,bytes:2000,missingDependencies:[],rightsStatus:'user_declared_authorized',rightsIndependentlyVerified:false,writesToLiveWorkspace:false,modelsActivated:false,measurementsCreated:false};
let service:ManagedService;
beforeEach(()=>{service={inventory:vi.fn().mockResolvedValue(inventory),preview:vi.fn().mockResolvedValue(preview),export:vi.fn().mockResolvedValue({...preview,writeVerified:true}),inspect:vi.fn().mockResolvedValue(preview),importIsolated:vi.fn().mockResolvedValue({...preview,isolationId:preview.packageSha256,idMapping:{[id]:'33333333-3333-4333-8333-333333333333'},status:'isolated_assets_stored',assetsRestoredToIsolation:true,assetsRestoredToLiveWorkspace:false,readyForComputation:false,reopenVerified:true})};});
async function select(){
  render(<ManagedPackagePanel projectId="project-a" service={service}/>);
  expect(service.inventory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button',{name:'读取受管资产清单'}));
  const option=await screen.findByRole('checkbox',{name:`选择资产 ${id}`});
  fireEvent.click(option);expect(screen.getByRole('button',{name:'预览所选受管资产'})).toBeDisabled();
  fireEvent.click(screen.getByRole('checkbox',{name:'确认所选资产及项目标题可分享，并具有打包权限'}));
}
async function prepare(){await select();fireEvent.click(screen.getByRole('button',{name:'预览所选受管资产'}));await screen.findByLabelText('受管资产导出预览');}
it('requires explicit asset selection and rights, then exports only the reviewed exact digest',async()=>{
  await prepare();
  expect(screen.getByRole('checkbox',{name:'选择资产 22222222-2222-4222-8222-222222222222'})).toBeDisabled();
  expect(service.preview).toHaveBeenCalledWith({projectId:'project-a',assetIds:[id],confirmedAssetRights:true});
  fireEvent.click(screen.getByRole('button',{name:'选择位置并导出受管资产包'}));
  await waitFor(()=>expect(service.export).toHaveBeenCalledWith({projectId:'project-a',assetIds:[id],confirmedAssetRights:true},preview.packageSha256));
  expect(await screen.findByRole('status')).toHaveTextContent('已保存并通过回读校验');
  expect(service.importIsolated).not.toHaveBeenCalled();
});
it('invalidates the preview and rights when asset selection changes',async()=>{
  await prepare();fireEvent.click(screen.getByRole('checkbox',{name:`选择资产 ${id}`}));
  expect(screen.queryByLabelText('受管资产导出预览')).not.toBeInTheDocument();
  expect(screen.getByRole('checkbox',{name:'确认所选资产及项目标题可分享，并具有打包权限'})).not.toBeChecked();
  expect(service.export).not.toHaveBeenCalled();
});
it('reports cancellation and hash conflicts without false success or private backend errors',async()=>{
  vi.mocked(service.export).mockResolvedValue(null);await prepare();
  fireEvent.click(screen.getByRole('button',{name:'选择位置并导出受管资产包'}));expect(await screen.findByRole('status')).toHaveTextContent('导出已取消');
  fireEvent.click(screen.getByRole('button',{name:'选择受管资产包并检查'}));await screen.findByLabelText('受管资产检查结果');
  vi.mocked(service.importIsolated).mockRejectedValue('package_isolation_conflict');
  fireEvent.click(screen.getByRole('checkbox',{name:'确认仅保存到独立隔离副本，不覆盖当前项目'}));
  fireEvent.click(screen.getByRole('button',{name:'重新选择同一包并保存隔离副本'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('未覆盖');expect(screen.queryByText(/重开与逐文件摘要校验通过/)).not.toBeInTheDocument();
});
it('requires confirmation and binds isolated import to the selected file hash',async()=>{
  render(<ManagedPackagePanel projectId="project-a" service={service}/>);
  fireEvent.click(screen.getByRole('button',{name:'选择受管资产包并检查'}));await screen.findByLabelText('受管资产检查结果');
  const button=screen.getByRole('button',{name:'重新选择同一包并保存隔离副本'});expect(button).toBeDisabled();
  fireEvent.click(screen.getByRole('checkbox',{name:'确认仅保存到独立隔离副本，不覆盖当前项目'}));fireEvent.click(button);
  await waitFor(()=>expect(service.importIsolated).toHaveBeenCalledWith(preview.packageSha256,true));
  expect(await screen.findByRole('status')).toHaveTextContent('未导入当前项目');
});
it('rejects unselected or changed assets and a receipt claiming to activate a live model',async()=>{
  vi.mocked(service.preview).mockResolvedValue({...preview,assets:[{...preview.assets[0],sha256:'e'.repeat(64)}]});
  await select();fireEvent.click(screen.getByRole('button',{name:'预览所选受管资产'}));expect(await screen.findByRole('alert')).toHaveTextContent('内容已变化');
  expect(screen.queryByLabelText('受管资产导出预览')).not.toBeInTheDocument();
  vi.mocked(service.inspect).mockResolvedValue({...preview,modelsActivated:true} as unknown as ManagedSummary);
  fireEvent.click(screen.getByRole('button',{name:'选择受管资产包并检查'}));
  await waitFor(()=>expect(screen.getByRole('alert')).toHaveTextContent('操作未完成'));
  expect(screen.queryByLabelText('受管资产检查结果')).not.toBeInTheDocument();
});
it('does not apply a late previous-project preview or show its data as a new-project success',async()=>{
  let resolve!:(v:ManagedPreview)=>void;vi.mocked(service.preview).mockImplementation(()=>new Promise(r=>{resolve=r;}));
  const view=render(<ManagedPackagePanel projectId="project-a" service={service}/>);
  fireEvent.click(screen.getByRole('button',{name:'读取受管资产清单'}));fireEvent.click(await screen.findByRole('checkbox',{name:`选择资产 ${id}`}));
  fireEvent.click(screen.getByRole('checkbox',{name:'确认所选资产及项目标题可分享，并具有打包权限'}));fireEvent.click(screen.getByRole('button',{name:'预览所选受管资产'}));
  await waitFor(()=>expect(service.preview).toHaveBeenCalled());view.rerender(<ManagedPackagePanel projectId="project-b" service={service}/>);
  await act(async()=>resolve(preview));
  expect(screen.queryByLabelText('受管资产导出预览')).not.toBeInTheDocument();expect(service.export).not.toHaveBeenCalled();
});
