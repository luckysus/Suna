import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {isolationLibraryService} from './isolationLibraryService';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.resetAllMocks();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});vi.mocked(invoke).mockResolvedValue(null);});
afterEach(()=>{delete (window as Window & {__TAURI_INTERNALS__?:unknown}).__TAURI_INTERNALS__;});
it('opens the existing host isolation by hash and offset without a caller path or database',async()=>{
  await isolationLibraryService.list(25);expect(invoke).toHaveBeenLastCalledWith('list_research_package_isolations',{offset:25});
  await isolationLibraryService.open('a'.repeat(64),50);expect(invoke).toHaveBeenLastCalledWith('open_research_package_isolation',{isolationId:'a'.repeat(64),offset:50});
});
