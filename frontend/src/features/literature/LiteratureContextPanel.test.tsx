import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import LiteratureContextPanel from './LiteratureContextPanel';
import type {LiteratureContextRecord,LiteratureContextRequest,LiteratureContextService,LiteratureContextSource} from '../../bridge/literatureContextTypes';
import type {LocalLiteratureEvidence} from '../../bridge/desktop';

const source:LiteratureContextSource={artifactId:'reviewed-evidence',evidenceSha256:'a'.repeat(64),field:'property.hardness_hv',value:500,unit:'HV',documentId:'same-document',versionId:'frozen-version',quote:'Hardness 500 HV.'};
const hit:LocalLiteratureEvidence={documentId:source.documentId,versionId:source.versionId,chunkId:'actual-chunk',chunkSha256:'b'.repeat(64),source:'Synthetic paper',quote:'Tensile tests followed ASTM E8 at 20 degC.',location:{page:2}};
let saved:LiteratureContextRecord[];
const service={sources:vi.fn(),list:vi.fn(),search:vi.fn(),save:vi.fn(),export:vi.fn()} satisfies LiteratureContextService;
beforeEach(()=>{
 vi.resetAllMocks();saved=[];
 service.sources.mockResolvedValue({readOnly:true,offset:0,hasMore:false,items:[source]});
 service.list.mockImplementation(async()=>({readOnly:true,offset:0,hasMore:false,items:saved}));
 service.search.mockResolvedValue([hit,{...hit,chunkId:'foreign-chunk',documentId:'other-document',source:'Different paper',location:{page:99}}]);
 service.export.mockResolvedValue(null);
 service.save.mockImplementation(async(_project:string,request:LiteratureContextRequest)=>{saved=[{artifactId:'saved-context',contentSha256:'c'.repeat(64),createdAt:'now',snapshot:{schemaVersion:'literature-experimental-context.v1',sourceType:'literature',reviewed:true,evidenceArtifactId:request.evidenceArtifactId,evidenceSha256:request.evidenceSha256,documentId:source.documentId,versionId:source.versionId,documentSha256:'d'.repeat(64),conditions:request.conditions.map(c=>({...c,location:hit.location})),scientificComparability:'not_assessed',rawSourceBytesVerified:false,unitsAndProcessOrderInferred:false}}];return {artifactId:'saved-context',contentSha256:'c'.repeat(64),sourceType:'literature',scientificComparability:'not_assessed'};});
});
function openPanel(){const summary=screen.getByText('文献实验条件与测试方法');if(!summary.closest('details')?.open)fireEvent.click(summary);}
async function chooseSource(){openPanel();fireEvent.click(screen.getByRole('button',{name:'读取已审核条件来源'}));await screen.findByLabelText('原数值证据');}
async function cite(){
 fireEvent.change(screen.getByLabelText('条件原文检索词'),{target:{value:'tensile'}});fireEvent.click(screen.getByRole('button',{name:'检索同版本条件片段'}));
 fireEvent.click(await screen.findByRole('button',{name:/选用条件片段/}));
 fireEvent.change(screen.getByLabelText('条件原文文字'),{target:{value:'ASTM E8'}});fireEvent.click(screen.getByRole('button',{name:'添加引用条件'}));
}
it('requires an existing project and performs no reads or writes before explicit actions',()=>{
 render(<LiteratureContextPanel projectId={null} service={service}/>);openPanel();expect(screen.getByRole('button',{name:'读取已审核条件来源'})).toBeDisabled();expect(service.sources).not.toHaveBeenCalled();expect(service.save).not.toHaveBeenCalled();
});
it('saves the exact reviewed source and cited same-version conditions, then reads persistent history and handles cancelled export',async()=>{
 render(<LiteratureContextPanel projectId="project" service={service}/>);await chooseSource();await cite();
 expect(screen.queryByRole('button',{name:/99/})).not.toBeInTheDocument();
 const save=screen.getByRole('button',{name:'确认并归档条件新版本'});expect(save).toBeDisabled();
 fireEvent.click(screen.getByLabelText('我已逐项核对条件文字、分类和原文引用'));fireEvent.click(save);
 expect(await screen.findByRole('status')).toHaveTextContent('saved-context');
 expect(service.save).toHaveBeenCalledWith('project',{evidenceArtifactId:source.artifactId,evidenceSha256:source.evidenceSha256,confirmed:true,conditions:[{kind:'test_method',text:'ASTM E8',quote:hit.quote,chunkId:hit.chunkId,chunkSha256:hit.chunkSha256}]});
 expect(service.list).toHaveBeenCalledWith('project',source.artifactId,0);expect(screen.getByText('引用在保存时核对；原文件与跨论文可比性未验证。')).toBeVisible();
 expect(screen.getByLabelText('我已逐项核对条件文字、分类和原文引用')).not.toBeChecked();
 fireEvent.click(screen.getByRole('button',{name:'导出当前页条件引用'}));expect(await screen.findByRole('status')).toHaveTextContent('导出已取消');
 const bytes=service.export.mock.calls[0][1] as number[];expect(new TextDecoder().decode(new Uint8Array(bytes))).toContain('actual-chunk');
});
it('clears confirmation when conditions change and refuses missing citation hashes',async()=>{
 render(<LiteratureContextPanel projectId="project" service={service}/>);await chooseSource();await cite();
 fireEvent.click(screen.getByLabelText('我已逐项核对条件文字、分类和原文引用'));fireEvent.click(screen.getByRole('button',{name:'移除条件 1'}));expect(screen.getByLabelText('我已逐项核对条件文字、分类和原文引用')).not.toBeChecked();
 service.search.mockResolvedValue([{...hit,chunkSha256:undefined}]);fireEvent.click(screen.getByRole('button',{name:'检索同版本条件片段'}));fireEvent.click(await screen.findByRole('button',{name:/选用条件片段/}));fireEvent.change(screen.getByLabelText('条件原文文字'),{target:{value:'ASTM E8'}});
 expect(screen.getByRole('button',{name:'添加引用条件'})).toBeDisabled();expect(service.save).not.toHaveBeenCalled();
});
it('does not present a stale source or a successful write after the project changes',async()=>{
 let release!:(value:unknown)=>void;service.sources.mockImplementation(()=>new Promise(resolve=>{release=resolve;}));
 const view=render(<LiteratureContextPanel projectId="old-project" service={service}/>);openPanel();fireEvent.click(screen.getByRole('button',{name:'读取已审核条件来源'}));
 view.rerender(<LiteratureContextPanel projectId="new-project" service={service}/>);
 await act(async()=>{release({readOnly:true,offset:0,hasMore:false,items:[source]});});
 expect(screen.queryByLabelText('原数值证据')).not.toBeInTheDocument();expect(screen.queryByRole('status')).not.toBeInTheDocument();expect(service.save).not.toHaveBeenCalled();
});
it('leaves the draft available and never reports success when native source validation rejects saving',async()=>{
 service.save.mockRejectedValue(new Error('synthetic-private-path-do-not-show'));
 render(<LiteratureContextPanel projectId="project" service={service}/>);await chooseSource();await cite();fireEvent.click(screen.getByLabelText('我已逐项核对条件文字、分类和原文引用'));fireEvent.click(screen.getByRole('button',{name:'确认并归档条件新版本'}));
 expect(await screen.findByRole('alert')).toHaveTextContent('未改写原记录');expect(screen.queryByText(/synthetic-private/)).not.toBeInTheDocument();expect(screen.queryByRole('status')).not.toBeInTheDocument();expect(screen.getByLabelText('我已逐项核对条件文字、分类和原文引用')).not.toBeChecked();
});
it('allows reading history for an archived project while all condition editing stays disabled',async()=>{
 render(<LiteratureContextPanel projectId="project" archived service={service}/>);await chooseSource();fireEvent.click(screen.getByRole('button',{name:'读取条件历史'}));await waitFor(()=>expect(service.list).toHaveBeenCalled());
 expect(screen.getByRole('button',{name:'确认并归档条件新版本'})).toBeDisabled();expect(screen.getByLabelText('条件原文检索词')).toBeDisabled();expect(service.save).not.toHaveBeenCalled();
});
