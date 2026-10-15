import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {collectProductionFiles,relativeImports} from './lib/production-graph.mjs';

test('lazy pages and their descendants remain inside the IPC security scan',()=>{
  const root=mkdtempSync(join(tmpdir(),'suna-production-graph-'));
  try{
    mkdirSync(join(root,'pages'));
    writeFileSync(join(root,'main.ts'),`import './shell'; const Page=deferredPage(()=>import('./pages/Research'));`);
    writeFileSync(join(root,'shell.ts'),`export {helper} from './state';`);
    writeFileSync(join(root,'state.ts'),`export const helper=1;`);
    writeFileSync(join(root,'pages/Research.tsx'),`import '../state'; const Details=()=>import('../forbidden');`);
    writeFileSync(join(root,'forbidden.ts'),`import {invoke} from '@tauri-apps/api/core'; export default invoke;`);
    const files=collectProductionFiles(join(root,'main.ts'));
    assert.deepEqual(files.sort(),['forbidden.ts','main.ts',join('pages','Research.tsx'),'shell.ts','state.ts'].map(p=>join(root,p)).sort());
    const violations=files.filter(p=>/@tauri-apps\/api\/(core|event)/.test(readFileSync(p,'utf8')));
    assert.deepEqual(violations,[join(root,'forbidden.ts')]);
  }finally{rmSync(root,{recursive:true,force:true});}
});
test('static exports, literal dynamic imports, duplicates and cycles remain bounded',()=>{
  assert.deepEqual(relativeImports(`import type {A} from './types';export {B} from './state';const one=()=>import( "./page" );const two=()=>import('./page');import React from 'react';`),['./types','./state','./page']);
  const root=mkdtempSync(join(tmpdir(),'suna-production-cycle-'));
  try{writeFileSync(join(root,'a.ts'),`import './b';`);writeFileSync(join(root,'b.ts'),`export const next=()=>import('./a');`);assert.equal(collectProductionFiles(join(root,'a.ts')).length,2);}
  finally{rmSync(root,{recursive:true,force:true});}
});
