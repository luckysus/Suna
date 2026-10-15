import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import IsolationLibrary from './IsolationLibrary';
import type {IsolationLibraryService,IsolationList,IsolationView} from '../../../bridge/isolationLibraryTypes';

const data='33333333-3333-4333-8333-333333333333',dataCopy='44444444-4444-4444-8444-444444444444';
const sha='a'.repeat(64),project='11111111-1111-4111-8111-111111111111',copy='22222222-2222-4222-8222-222222222222';
const page:IsolationList={schemaVersion:'research-managed-isolations.v1',readOnly:true,writesToLiveWorkspace:false,offset:0,total:2,hasMore:false,
  items:[{isolationId:sha,state:'stored_unverified'},{isolationId:'b'.repeat(64),state:'requires_original_archive'}]};
const view:IsolationView={schemaVersion:'research-managed-isolation-view.v1',readOnly:true,offset:0,total:2,hasMore:false,
  isolation:{schemaVersion:'research-managed-package.v1',scope:'managed_assets_isolated',packageSha256:sha,sourceProjectId:project,isolationId:sha,idMapping:{[project]:copy,[data]:dataCopy},status:'isolated_assets_stored',assetsRestoredToIsolation:true,assetsRestoredToLiveWorkspace:false,readyForComputation:false,reopenVerified:true,originalArchiveStored:true,
    metadata:{schemaVersion:'research-package-manifest.v1',scope:'metadata_only',sourceProjectId:project,receiptSha256:'c'.repeat(64),metadataRecordCount:2,edgeCount:0,missingDependencies:[],assetsIncluded:false,provenanceMode:'references_only',rightsStatus:'not_assessed',writesToLiveWorkspace:false,assetsRestored:false},
    assets:[{resourceId:data,role:'dataset_source',extension:'csv',sha256:'e'.repeat(64),bytes:20}],assetCount:1,assetBytes:20,missingDependencies:[],rightsStatus:'user_declared_authorized',rightsIndependentlyVerified:false,writesToLiveWorkspace:false,modelsActivated:false,measurementsCreated:false},
  records:[{originalId:project,importedId:copy,recordSha256:'d'.repeat(64),record:{id:project,kind:'project',name:'合成隔离课题'}},{originalId:data,importedId:dataCopy,recordSha256:'f'.repeat(64),record:{id:data,kind:'dataset',name:'合成输入'}}]};
let service:IsolationLibraryService;
beforeEach(()=>{service={list:vi.fn().mockResolvedValue(page),open:vi.fn().mockResolvedValue(view)};});
async function load(){render(<IsolationLibrary service={service}/>);expect(service.list).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'读取隔离副本列表'}));await screen.findByText('已保存，尚未重新校验');}
async function open(){await load();fireEvent.click(screen.getByRole('button',{name:'校验并打开 aaaaaaaa'}));await screen.findByLabelText('已校验隔离元数据');}
it('lazily lists stored states and opens only a verified read-only native receipt',async()=>{
  await open();expect(service.list).toHaveBeenCalledWith(0);expect(service.open).toHaveBeenCalledWith(sha,0);
  expect(screen.getByRole('button',{name:'校验并打开 bbbbbbbb'})).toBeDisabled();
  expect(screen.getByText('合成隔离课题')).toBeInTheDocument();expect(screen.getByRole('status')).toHaveTextContent('未导入当前项目');
  expect(screen.getByText(`元数据记录摘要 ${'d'.repeat(64)}`)).toBeInTheDocument();
});
it('rejects live activation claims and UUID mappings that do not bind the returned records',async()=>{
  vi.mocked(service.open).mockResolvedValue({...view,isolation:{...view.isolation,modelsActivated:true}} as unknown as IsolationView);
  await load();fireEvent.click(screen.getByRole('button',{name:'校验并打开 aaaaaaaa'}));expect(await screen.findByRole('alert')).toHaveTextContent('未完成');
  expect(screen.queryByLabelText('已校验隔离元数据')).not.toBeInTheDocument();
  vi.mocked(service.open).mockResolvedValue({...view,records:[{...view.records[0],importedId:project}]});
  fireEvent.click(screen.getByRole('button',{name:'校验并打开 aaaaaaaa'}));await waitFor(()=>expect(service.open).toHaveBeenCalledTimes(2));
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
});
it('does not display raw backend details or claim a legacy copy has reopened',async()=>{
  vi.mocked(service.open).mockRejectedValue('package_isolation_original_required');await load();
  fireEvent.click(screen.getByRole('button',{name:'校验并打开 aaaaaaaa'}));expect(await screen.findByRole('alert')).toHaveTextContent('重新选择原包');
  vi.mocked(service.open).mockRejectedValue('synthetic-private-server-detail');
  fireEvent.click(screen.getByRole('button',{name:'校验并打开 aaaaaaaa'}));await waitFor(()=>expect(screen.getByRole('alert')).toHaveTextContent('未完成'));
  expect(screen.queryByText(/synthetic-private/)).not.toBeInTheDocument();expect(screen.queryByRole('status')).not.toBeInTheDocument();
});
it('uses bounded offsets for the persistent library and never silently accepts a partial page',async()=>{
  const items=Array.from({length:26},(_,i)=>({isolationId:i.toString(16).padStart(64,'0'),state:'stored_unverified' as const}));
  vi.mocked(service.list).mockResolvedValueOnce({...page,total:26,hasMore:true,items:items.slice(0,25)}).mockResolvedValueOnce({...page,offset:25,total:26,items:items.slice(25)});
  render(<IsolationLibrary service={service}/>);fireEvent.click(screen.getByRole('button',{name:'读取隔离副本列表'}));await screen.findByText('共 26 个副本，本页 25 个；均需重新校验。');
  fireEvent.click(screen.getByRole('button',{name:'更多隔离副本'}));await screen.findByText('共 26 个副本，本页 1 个；均需重新校验。');expect(service.list).toHaveBeenLastCalledWith(25);
  vi.mocked(service.list).mockResolvedValue({...page,total:26,hasMore:true});fireEvent.click(screen.getByRole('button',{name:'读取隔离副本列表'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('未完成');expect(screen.queryByText('共 26 个副本，本页 2 个；均需重新校验。')).not.toBeInTheDocument();
});
it('paginates metadata for the exact stored package and retains the original-to-new mapping',async()=>{
  const rows=Array.from({length:26},(_,i)=>{const id=`${i.toString().padStart(8,'0')}-1111-4111-8111-111111111111`,mapped=`${i.toString().padStart(8,'0')}-2222-4222-8222-222222222222`;return {originalId:id,importedId:mapped,recordSha256:'d'.repeat(64),record:{id,kind:'dataset',name:`合成元数据 ${i}`}};});
  const isolation={...view.isolation,idMapping:Object.fromEntries(rows.map(r=>[r.originalId,r.importedId])),metadata:{...view.isolation.metadata,metadataRecordCount:26}};
  vi.mocked(service.open).mockResolvedValueOnce({...view,isolation,total:26,hasMore:true,records:rows.slice(0,25)}).mockResolvedValueOnce({...view,isolation,offset:25,total:26,records:rows.slice(25)});
  await open();fireEvent.click(screen.getByRole('button',{name:'更多隔离元数据'}));expect(await screen.findByText('合成元数据 25')).toBeInTheDocument();expect(service.open).toHaveBeenLastCalledWith(sha,25);expect(screen.queryByText('合成元数据 0')).not.toBeInTheDocument();
});
it('discards a late reply after the library is unmounted and reopened',async()=>{
  let resolve!:(v:IsolationList)=>void;vi.mocked(service.list).mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));
  const first=render(<IsolationLibrary service={service}/>);fireEvent.click(screen.getByRole('button',{name:'读取隔离副本列表'}));first.unmount();
  render(<IsolationLibrary service={service}/>);await act(async()=>resolve(page));expect(screen.queryByText('已保存，尚未重新校验')).not.toBeInTheDocument();expect(service.open).not.toHaveBeenCalled();
});
