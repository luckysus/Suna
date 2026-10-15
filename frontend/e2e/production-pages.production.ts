import {expect,test,type Page} from '@playwright/test';

async function fixture(page:Page){
  await page.route('**/*',route=>new URL(route.request().url()).hostname==='127.0.0.1'?route.continue():route.abort());
  await page.addInitScript(()=>{
    Object.assign(window,{__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}}});
    window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback:()=>{},
      invoke:async(command:string,args?:Record<string,unknown>)=>{
        if(command==='plugin:event|listen')return 1;
        if(command==='get_setting')return args?.key==='ui.preferences'?JSON.stringify({autoUpdate:false,animationEnabled:false}):null;
        if(command==='list_skills')return {skills:[],warnings:[]};
        if(command.startsWith('list_')||command==='recover_agent_runs'||command.startsWith('search_'))return [];
        return null;
      }};
  });
}

test('production navigation loads page code only when requested',async({page},info)=>{
  await fixture(page);const scripts=new Map<string,number>(),pending:Promise<void>[]=[];
  page.on('response',response=>{if(response.request().resourceType()==='script')pending.push(response.body().then(b=>{scripts.set(new URL(response.url()).pathname,b.length);}).catch(()=>{}));});
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto('/');await expect(page.getByRole('heading',{name:/你好，我是 Suna/})).toBeVisible();await page.waitForLoadState('networkidle');await Promise.all(pending);
  const initial=[...scripts].map(([path,bytes])=>({path,bytes}));
  expect(initial.some(s=>/PerformancePredictionPage|ProcessOptimizationPage|DataLabPage/.test(s.path))).toBe(false);
  await info.attach('initial-javascript.json',{body:JSON.stringify({initialScripts:initial,decodedJavaScriptBytes:initial.reduce((n,s)=>n+s.bytes,0)},null,2),contentType:'application/json'});
  await page.getByRole('button',{name:'性能预测',exact:true}).click();await expect(page.getByRole('heading',{name:'性能预测',exact:true})).toBeVisible();
  await page.waitForLoadState('networkidle');await Promise.all(pending);
  expect([...scripts.keys()].some(path=>/PerformancePredictionPage-[^/]+\.js$/.test(path))).toBe(true);
  expect(errors).toEqual([]);
});

test('a failed local page asset exposes recovery while keeping the shell',async({page})=>{
  await fixture(page);let blocked=false;
  await page.route('**/assets/ProcessOptimizationPage-*.js',route=>{if(!blocked){blocked=true;return route.abort();}return route.continue();});
  await page.goto('/');await expect(page.getByRole('heading',{name:/你好，我是 Suna/})).toBeVisible();
  await page.getByRole('button',{name:'工艺优化',exact:true}).click();
  await expect(page.getByRole('alert')).toContainText('页面加载失败');await expect(page.locator('.suna-new-sidebar')).toBeVisible();
  await page.getByRole('button',{name:'重新加载页面',exact:true}).click();
  await expect(page.getByRole('heading',{name:'工艺优化',exact:true})).toBeVisible();expect(blocked).toBe(true);
});
