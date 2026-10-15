import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {evidenceQualityService} from './evidenceQualityService';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.resetAllMocks();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});vi.mocked(invoke).mockResolvedValue(null);});
afterEach(()=>{delete (window as Window & {__TAURI_INTERNALS__?:unknown}).__TAURI_INTERNALS__;});
it('uses the registered project and immutable report hash contract without submitting evidence or credentials',async()=>{
  await evidenceQualityService.preview('project-a');expect(invoke).toHaveBeenLastCalledWith('preview_literature_evidence_quality',{projectId:'project-a'});
  await evidenceQualityService.archive('project-a','a'.repeat(64));expect(invoke).toHaveBeenLastCalledWith('archive_literature_evidence_quality',{projectId:'project-a',expectedSha256:'a'.repeat(64)});
});
