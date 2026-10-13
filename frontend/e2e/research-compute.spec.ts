import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const column = (ordinal:number,name:string,min:number,max:number) => ({ordinal,originalName:name,canonicalField:name,inferredType:"number",duplicate:false,min,max,unit:null,nonEmptyCount:50,missingCount:0,invalidCount:0});
    const dataset={id:"dataset",sourceName:"steel.csv",mappingState:"ready",rowCount:50,columns:[{...column(0,"heat",0,0),inferredType:"text"},column(3,"C",.1,.5),column(8,"temperature",700,1000),column(10,"HV",100,500),column(11,"Fe",98,100)]};
    const artifact=(id:string,names:string[])=>({task_id:id,state:"completed",artifact:{model_id:id,model_type:id==="hardness"?"random_forest":"svr",feature_names:names,applicability_range:names.map(()=>({min:0,max:1000})),preprocessing:{means:id==="hardness"?[850,.3]:[.3,850],scales:[1,1]},metrics:{}}});
    const trainings=["hardness","toughness"].map((id)=>({id,kind:"compute_train_sklearn_model",state:"completed",dataset_id:"dataset",updated_at:"2026-10-13",progress:100,error_code:null}));
    let submitted:Record<string,unknown>|null=null;
    const predictions=new Map<string,number[][]>();
    const result={task_id:"optimization",method:"nsga2",objectives:["HV","J"],directions:["maximize","minimize"],feature_names:["temperature","C"],trials_completed:80,deterministic_seed:42,model_type:"joint",model_id:"joint",recommendations:[{values:{temperature:850,C:.3},objectives:[300,50],prediction:300,feasible:true,constraint_residuals:{}}],pareto_front:[]};
    result.pareto_front=result.recommendations as never;
    Object.assign(window,{__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}}});
    window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:"main"},currentWebview:{label:"main"}},transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command:string,args?:Record<string,unknown>)=>{
      if(command==="plugin:event|listen")return 1;
      if(command==="plugin:event|unlisten"||command==="db_init"||command==="set_setting")return null;
      if(command==="get_setting")return null;
      if(command==="list_steel_datasets")return [dataset];
      if(command==="list_background_tasks")return [...trainings,...(submitted?[{id:"optimization",kind:"compute_optimize_constrained",state:"completed",progress:100,error_code:null}]:[]),...Array.from(predictions.keys()).map((id)=>({id,kind:"compute_predict_trained",state:"completed",progress:100,error_code:null}))];
      if(command==="get_compute_training_result")return artifact(String(args?.id),args?.id==="hardness"?["temperature","C"]:["C","temperature"]);
      if(command==="train_steel_dataset"){Object.assign(window,{trainingRequest:args?.request});return {id:"hardness"};}
      if(command==="register_steel_model")return {version:1};
      if(command==="predict_steel_model"){
        const request=args?.request as {featureRows?:number[][];featureValues?:number[]};
        const id=`prediction-${predictions.size+1}`;predictions.set(id,request.featureRows??[request.featureValues??[]]);Object.assign(window,{predictionRequest:request});return {id};
      }
      if(command==="get_compute_prediction_result"){
        const rows=predictions.get(String(args?.id));if(!rows)return null;
        return {task_id:args?.id,state:"completed",model_id:"hardness",model_type:"random_forest",feature_names:["temperature","C"],predictions:rows.map(([t,c])=>t*.1+c*1000),applicability_warnings:[],applicability_range:[{min:700,max:1000},{min:.1,max:.5}],confidence:null};
      }
      if(command==="optimize_steel_process"){submitted=args?.request as Record<string,unknown>;Object.assign(window,{optimizationRequest:submitted});return {id:"optimization"};}
      if(command==="get_compute_optimization_result")return submitted?result:null;
      if(command==="save_export_file"){Object.assign(window,{exportRequest:args});return {path:"C:\\研究\\优化.json",bytes:10};}
      return [];
    }};
  });
});

test("multi-performance optimization submits independent models and named constraints", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button",{name:"工艺优化",exact:true}).click();
  await expect(page.getByLabel("目标 1 名称")).toBeVisible();
  await page.getByLabel("目标 1 名称").fill("HV");
  await page.getByRole("button",{name:"添加性能目标"}).click();
  await page.getByLabel("目标 2 名称").fill("J");
  await page.getByLabel("目标 2 模型").selectOption("toughness");
  await page.getByLabel("目标 2 方向").selectOption("minimize");
  await page.getByLabel("添加设计变量").selectOption("Fe");
  await page.getByLabel("Fe 固定值").fill("99.7");
  await page.getByLabel("C 固定值").fill("0.3");
  await page.getByRole("button",{name:"添加配比约束"}).click();
  await page.getByLabel("C 系数",{exact:true}).fill("1");
  await page.getByLabel("阈值",{exact:true}).fill("0.3");
  await page.getByRole("button",{name:"开始优化"}).click();
  await expect(page.getByText("300.000",{exact:true})).toBeVisible();
  const request=await page.evaluate(()=> (window as unknown as {optimizationRequest:Record<string,unknown>}).optimizationRequest);
  expect(request.objectives).toMatchObject([{modelId:"hardness",direction:"maximize"},{modelId:"toughness",direction:"minimize"}]);
  expect(request.variables).toMatchObject([{name:"temperature"},{name:"C",fixedValue:.3},{name:"Fe",fixedValue:99.7}]);
  expect(request.constraints).toMatchObject([{coefficients:{C:1},value:.3}]);
  await page.getByRole("button",{name:"导出 JSON"}).click();
  await expect(page.getByText(/候选方案已导出/)).toBeVisible();
  expect(await page.evaluate(()=> (window as unknown as {exportRequest:{bytes:number[]}}).exportRequest.bytes.length)).toBeGreaterThan(0);
});


test("scientific split configuration, nonlinear attribution and batch export work through native commands", async ({page}) => {
  await page.goto("/");
  await page.getByRole("button",{name:"性能预测",exact:true}).click();
  await page.getByLabel("模型",{exact:true}).click();
  await page.getByRole("option",{name:/Random Forest/}).click();
  await page.getByLabel("预测目标",{exact:true}).click();
  await page.getByRole("option",{name:/硬度/}).click();
  await page.getByLabel("划分策略").selectOption("group");
  await page.getByLabel("分组或时间列").selectOption("0");
  await page.getByLabel("交叉验证折数").fill("3");
  await page.getByRole("button",{name:"训练模型",exact:true}).click();
  await expect(page.getByRole("button",{name:"解释本次输入"})).toBeVisible();
  const training=await page.evaluate(()=> (window as unknown as {trainingRequest:Record<string,unknown>}).trainingRequest);
  expect(training).toMatchObject({targetColumn:10,featureColumns:[3,8],algorithm:"random_forest",splitPolicy:{kind:"group",groupColumn:0,folds:3,testFraction:.2,calibrationFraction:.2}});
  const inputs=page.locator(".suna-prediction-inputs input");
  await inputs.nth(0).fill("900");await inputs.nth(1).fill("0.4");
  await page.getByRole("button",{name:"解释本次输入"}).click();
  await expect(page.getByText(/枚举基线 Shapley/)).toBeVisible();
  await expect(page.getByText("+100.000",{exact:true})).toBeVisible();
  await page.getByRole("button",{name:"批量预测",exact:true}).click();
  await page.getByLabel("批量预测输入").fill('[{"C":0.4,"temperature":900},{"C":0.2,"temperature":800}]');
  await page.getByRole("button",{name:"提交整批预测"}).click();
  await expect(page.getByRole("cell",{name:"490",exact:true})).toBeVisible();
  const prediction=await page.evaluate(()=> (window as unknown as {predictionRequest:Record<string,unknown>}).predictionRequest);
  expect(prediction.featureRows).toEqual([[900,.4],[800,.2]]);
  await page.getByRole("button",{name:"导出预测 CSV"}).click();
  await expect(page.getByText(/批量预测已导出/)).toBeVisible();
});
