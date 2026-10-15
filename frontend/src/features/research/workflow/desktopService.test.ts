import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {workflowService} from './desktopService';
import type {Template} from './types';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn()}));
beforeEach(()=>{vi.resetAllMocks();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});});
afterEach(()=>{delete (window as Window & {__TAURI_INTERNALS__?:unknown}).__TAURI_INTERNALS__;});
it('uses the registered desktop commands and frozen selection with no raw template or Worker permit in the preview request',async()=>{
  const template={id:'template',revision:2} as Template;
  const input={projectId:'project',datasetId:'dataset',datasetSha256:'a'.repeat(64)};
  vi.mocked(invoke).mockResolvedValue({dryRunSha256:'b'.repeat(64)});
  await workflowService.dryRun(template,input);
  expect(invoke).toHaveBeenCalledWith('preview_research_workflow',{request:{...input,templateId:'template',templateRevision:2}});
  const start={...input,templateId:'template',templateRevision:2,dryRunSha256:'b'.repeat(64),nonce:'nonce',confirmed:true as const};
  await workflowService.start(start);expect(invoke).toHaveBeenLastCalledWith('start_research_workflow',{request:start});
  await workflowService.listRuns('project');expect(invoke).toHaveBeenLastCalledWith('list_research_workflows',{projectId:'project',limit:25,offset:0});
});
it('projects dataset metadata and subscribes through the original Scheduler event',async()=>{
  vi.mocked(invoke).mockResolvedValue([{id:'dataset',sourceName:'合成.csv',sourceSha256:'a'.repeat(64),mappingState:'ready',columns:[],sourcePath:'C:/synthetic-private/source.csv',preview:{rows:[['synthetic-raw-row']]}}]);
  expect(await workflowService.listDatasets()).toEqual([{id:'dataset',sourceName:'合成.csv',sourceSha256:'a'.repeat(64),mappingState:'ready',columns:[]}]);
  const receive=vi.fn(),unsubscribe=vi.fn();vi.mocked(listen).mockResolvedValue(unsubscribe);
  const stop=await workflowService.listenSchedulerProgress(receive);
  expect(listen).toHaveBeenCalledWith('scheduler:progress',expect.any(Function));
  const listener=vi.mocked(listen).mock.calls[0][1];listener({event:'scheduler:progress',id:1,payload:{Progress:{id:'root',state:'completed'}}});
  expect(receive).toHaveBeenCalledWith({id:'root',state:'completed'});stop();expect(unsubscribe).toHaveBeenCalledOnce();
});
it('passes each complete scientific policy to the existing immutable template command',async()=>{
  const policy={kind:'group' as const,groupColumn:4,validationFraction:0.2,testFraction:0.15,calibrationFraction:0.2,folds:3,searchTrials:2,learningCurve:true,seed:7};
  const template={id:'template',revision:2,models:[{splitPolicy:policy},{splitPolicy:{...policy,kind:'time' as const,groupColumn:undefined,timeColumn:5,seed:17}}]} as Template;
  vi.mocked(invoke).mockResolvedValue({template,repositoryDryRunRequired:true});
  await workflowService.saveTemplate(template,2);
  expect(invoke).toHaveBeenCalledWith('save_research_workflow_template',{template,expectedRevision:2});
});
