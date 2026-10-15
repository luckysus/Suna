import {expect,test} from '@playwright/test';

test('cited conditions use the real desktop bridge only after explicit review',async({page})=>{
 await page.route('**/*',route=>new URL(route.request().url()).hostname==='127.0.0.1'?route.continue():route.abort());
 await page.addInitScript(()=>{
  const project={id:'00000000-0000-4000-8000-000000000101',name:'Synthetic context project',description:'',materialSystem:null,status:'active',tags:[],createdAt:'fixture',updatedAt:'fixture'};
  const source={artifactId:'00000000-0000-4000-8000-000000000102',evidenceSha256:'a'.repeat(64),field:'property.hardness_hv',value:500,unit:'HV',documentId:'00000000-0000-4000-8000-000000000103',versionId:'00000000-0000-4000-8000-000000000104',quote:'Synthetic hardness 500 HV.'};
  const hit={documentId:source.documentId,versionId:source.versionId,chunkId:'chunk-'+'b'.repeat(64),chunkSha256:'b'.repeat(64),quote:'Synthetic tensile test used ASTM E8.',source:'Synthetic text',location:{kind:'heading',path:['Methods']}};
  const records:unknown[]=[];Object.assign(window,{contextWrites:[],__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}}});
  window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command:string,args?:Record<string,unknown>)=>{
   if(command==='plugin:event|listen')return 1;
   if(command==='get_setting')return null;
   if(command==='get_current_research_project')return project;
   if(command==='list_research_projects')return [project];
   if(command==='list_skills')return {skills:[],warnings:[]};
   if(command==='list_literature_context_sources')return {readOnly:true,offset:0,hasMore:false,items:[source]};
   if(command==='search_local_literature_evidence')return [hit];
   if(command==='archive_literature_experimental_context'){
    (window as unknown as {contextWrites:unknown[]}).contextWrites.push(args);
    const request=args?.request as {conditions:Record<string,unknown>[]};
    records.push({artifactId:'saved-context',contentSha256:'c'.repeat(64),createdAt:'fixture',snapshot:{schemaVersion:'literature-experimental-context.v1',sourceType:'literature',reviewed:true,evidenceArtifactId:source.artifactId,evidenceSha256:source.evidenceSha256,documentId:source.documentId,versionId:source.versionId,documentSha256:'d'.repeat(64),conditions:request.conditions.map(c=>({...c,location:hit.location})),scientificComparability:'not_assessed',rawSourceBytesVerified:false,unitsAndProcessOrderInferred:false}});
    return {artifactId:'saved-context',contentSha256:'c'.repeat(64),sourceType:'literature',scientificComparability:'not_assessed'};
   }
   if(command==='list_literature_experimental_contexts')return {readOnly:true,offset:0,hasMore:false,items:records};
   if(command.startsWith('list_')||command.startsWith('search_')||command==='recover_agent_runs')return [];
   return null;
  }};
 });
 await page.goto('/literature');await page.getByRole('button',{name:'公开发现 / 本地题录'}).click();
 await page.getByText('文献实验条件与测试方法',{exact:true}).click();
 await page.getByRole('button',{name:'读取已审核条件来源'}).click();
 await page.getByLabel('条件原文检索词').fill('ASTM');await page.getByRole('button',{name:'检索同版本条件片段'}).click();
 await page.getByRole('button',{name:/选用条件片段/}).click();await page.getByLabel('条件原文文字',{exact:true}).fill('ASTM E8');await page.getByRole('button',{name:'添加引用条件'}).click();
 await expect(page.getByRole('button',{name:'确认并归档条件新版本'})).toBeDisabled();expect(await page.evaluate(()=>(window as unknown as {contextWrites:unknown[]}).contextWrites.length)).toBe(0);
 await page.getByLabel('我已逐项核对条件文字、分类和原文引用').check();await page.getByRole('button',{name:'确认并归档条件新版本'}).click();
 await expect(page.getByRole('status').filter({hasText:'实验条件已保存'})).toBeVisible();await expect(page.getByText('引用在保存时核对；原文件与跨论文可比性未验证。')).toBeVisible();
 const writes=await page.evaluate(()=>(window as unknown as {contextWrites:unknown[]}).contextWrites);
 expect(writes).toEqual([{projectId:'00000000-0000-4000-8000-000000000101',request:{evidenceArtifactId:'00000000-0000-4000-8000-000000000102',evidenceSha256:'a'.repeat(64),confirmed:true,conditions:[{kind:'test_method',text:'ASTM E8',quote:'Synthetic tensile test used ASTM E8.',chunkId:'chunk-'+'b'.repeat(64),chunkSha256:'b'.repeat(64)}]}}]);
});
