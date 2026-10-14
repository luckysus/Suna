import {desktop} from '../../../bridge/desktop';
import type {Service} from './types';
// Stable adapter: all IPC and task events use the existing desktop bridge.
export const workflowService:Service={
  async listDatasets(){
    return (await desktop.listSteelDatasets()).map(({id,sourceName,sourceSha256,mappingState,columns})=>({id,sourceName,sourceSha256,mappingState,columns}));
  },
  listTemplates:()=>desktop.listResearchWorkflowTemplates(),
  getTemplate:(id,revision)=>desktop.getResearchWorkflowTemplate(id,revision),
  saveTemplate:(template,expected)=>desktop.saveResearchWorkflowTemplate(template,expected),
  dryRun:(template,input)=>desktop.previewResearchWorkflow({...input,templateId:template.id,templateRevision:template.revision}),
  start:request=>desktop.startResearchWorkflow(request),
  listRuns:project=>desktop.listResearchWorkflows(project),
  getRun:(project,id)=>desktop.getResearchWorkflow(project,id),
  cancelBackgroundTask:id=>desktop.cancelBackgroundTask(id),
  retryBackgroundTask:id=>desktop.retryBackgroundTask(id),
  listenSchedulerProgress:fn=>desktop.listenSchedulerProgress(fn),
};
