import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {managedService} from './managedService';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.resetAllMocks();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});vi.mocked(invoke).mockResolvedValue(null);});
afterEach(()=>{delete (window as Window & {__TAURI_INTERNALS__?:unknown}).__TAURI_INTERNALS__;});
it('uses registered native asset commands with rights/hash binding and never submits a user file path',async()=>{
  const request={projectId:'project-a',assetIds:['asset-a'],confirmedAssetRights:true};
  await managedService.inventory('project-a');expect(invoke).toHaveBeenLastCalledWith('list_research_managed_assets',{projectId:'project-a'});
  await managedService.preview(request);expect(invoke).toHaveBeenLastCalledWith('preview_research_managed_package',{request});
  await managedService.export(request,'a'.repeat(64));expect(invoke).toHaveBeenLastCalledWith('export_research_managed_package',{request,expectedSha256:'a'.repeat(64)});
  await managedService.inspect();expect(invoke).toHaveBeenLastCalledWith('inspect_research_managed_package_file',{});
  await managedService.importIsolated('a'.repeat(64),true);expect(invoke).toHaveBeenLastCalledWith('import_research_managed_package_isolated',{expectedSha256:'a'.repeat(64),confirmed:true});
});
