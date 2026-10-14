import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {packageService} from './desktopService';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.resetAllMocks();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});});
afterEach(()=>{delete (window as Window & {__TAURI_INTERNALS__?:unknown}).__TAURI_INTERNALS__;});
it('uses actual registered package commands, binds the preview digest and leaves file selection with the host',async()=>{
  vi.mocked(invoke).mockResolvedValue(null);
  await packageService.preview('project-a');expect(invoke).toHaveBeenLastCalledWith('preview_research_package',{projectId:'project-a'});
  await packageService.export('project-a','a'.repeat(64));expect(invoke).toHaveBeenLastCalledWith('export_research_package',{projectId:'project-a',expectedSha256:'a'.repeat(64)});
  await packageService.inspectSelectedFile();expect(invoke).toHaveBeenLastCalledWith('inspect_research_package_file',{});
});
