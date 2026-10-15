import {act, fireEvent, render, screen, waitFor, within} from '@testing-library/react';
import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import DataLabPage from './DataLabPage';
import type {SteelDatasetRecord} from '../../bridge/desktop';
import {desktop} from '../../bridge/desktop';
import {useResearchStore} from '../../stores/researchStore';

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
// Native drag/drop and file dialogs require personal Windows acceptance.
vi.mock('../../hooks/useNativeFileDrop',()=>({useNativeFileDrop:vi.fn()}));
const original={id:'raw',sourceName:'原始合成.csv',sourcePath:'/synthetic/raw.csv',sourceSha256:'a'.repeat(64),
  format:'csv',selectedSheet:'CSV',rowCount:12,columnCount:1,truncated:false,mappingState:'draft',
  columns:[{ordinal:0,originalName:'C',inferredType:'number',canonicalField:null,unit:null,duplicate:false,
    min:0.1,max:0.2,nonEmptyCount:12,missingCount:0,invalidCount:0}],preview:{},createdAt:'now',updatedAt:'now'} as SteelDatasetRecord;
const normalized={...original,id:'normalized',sourceName:'规范化合成.csv',sourcePath:'/synthetic/normalized.csv',sourceSha256:'b'.repeat(64),
  columns:[{...original.columns[0],canonicalField:'chem.c',unit:'wt%'}]};
let datasets:SteelDatasetRecord[];
async function mount(){
  // Flush the synthetic native listing and the resulting dataset selection.
  await act(async()=>{render(<DataLabPage/>);});
}
async function confirmActivation(){
  const review=screen.getByLabelText('我已核对当前版本的列映射和单位');
  expect(review).toBeEnabled();
  await act(async()=>{fireEvent.click(review);});
  expect(review).toBeChecked();
  const button=screen.getByRole('button',{name:'确认激活此数据集'});
  expect(button).toBeEnabled();
  await act(async()=>{fireEvent.click(button);});
}
beforeEach(()=>{
  vi.resetAllMocks();window.history.replaceState({},'', '/');
  Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});
  useResearchStore.setState({currentProject:{id:'project',name:'合成项目',description:'',materialSystem:null,status:'active',tags:[],createdAt:'now',updatedAt:'now'}});
  datasets=[original];
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==='list_steel_datasets')return datasets;
    if(command==='get_material_registry')return {fields:[{id:'chem.c',canonicalUnit:'wt%',role:'composition',valueType:'float'}]};
    if(command==='suggest_material_fields')return [[]];
    if(command==='normalize_material_dataset'){datasets=[normalized,...datasets];return normalized;}
    if(command==='activate_steel_dataset'){
      const id=(args as {datasetId:string}).datasetId;
      datasets=datasets.map(d=>d.id===id?{...d,mappingState:'ready'}:d);
      return datasets.find(d=>d.id===id);
    }
    throw new Error('unexpected fixture command');
  });
});
afterEach(()=>{delete (window as Window & {__TAURI_INTERNALS__?:unknown}).__TAURI_INTERNALS__;});
it('selects the persisted normalized version and explicitly activates that exact ID through the existing native bridge',async()=>{
  await mount();
  const mapping=await screen.findByLabelText('C 规范字段');
  await waitFor(()=>expect(within(mapping).getByRole('option',{name:/chem.c/})).toBeVisible());
  fireEvent.change(mapping,{target:{value:'chem.c'}});
  fireEvent.change(screen.getByLabelText('C 原始单位'),{target:{value:'wt%'}});
  await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'确认映射与单位，创建新版本'}));});
  expect(await screen.findByRole('heading',{name:'规范化合成.csv'})).toBeVisible();
  expect(invoke).toHaveBeenCalledWith('normalize_material_dataset',{request:{projectId:'project',datasetId:'raw',mappings:[{ordinal:0,fieldId:'chem.c',sourceUnit:'wt%'}]}});
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='activate_steel_dataset')).toHaveLength(0);
  expect(screen.getByRole('button',{name:'确认激活此数据集'})).toBeDisabled();
  await confirmActivation();
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith('activate_steel_dataset',{datasetId:'normalized',review:{sourceSha256:normalized.sourceSha256,updatedAt:normalized.updatedAt}}));
  expect(await screen.findByText('此数据集已激活，可在预测和科研工作流中选择。')).toBeVisible();
  expect(datasets.find(d=>d.id==='raw')?.mappingState).toBe('draft');
  expect(datasets.find(d=>d.id==='normalized')?.sourceSha256).toBe(normalized.sourceSha256);
});
it('leaves a failed activation as draft and never displays a successful activation',async()=>{
  datasets=[normalized];
  vi.mocked(invoke).mockImplementation(async command=>{
    if(command==='list_steel_datasets')return datasets;
    if(command==='get_material_registry')return {fields:[]};
    if(command==='suggest_material_fields')return [[]];
    if(command==='activate_steel_dataset')throw new Error('steel dataset source changed since it was saved');
    throw new Error('unexpected fixture command');
  });
  await mount();
  await confirmActivation();
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith('activate_steel_dataset',{datasetId:normalized.id,review:{sourceSha256:normalized.sourceSha256,updatedAt:normalized.updatedAt}}));
  expect(await screen.findByRole('alert')).toHaveTextContent('source changed');
  expect(screen.queryByText('此数据集已激活，可在预测和科研工作流中选择。')).toBeNull();
  expect(datasets[0].mappingState).toBe('draft');
});
it('ignores a late activation reply after another dataset is selected',async()=>{
  const second={...normalized,id:'second',sourceName:'第二合成.csv'};
  datasets=[normalized,second];
  let resolve!:(value:unknown)=>void;
  const pending=new Promise(r=>{resolve=r;});
  const originalInvoke=vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command,args)=>command==='activate_steel_dataset'?pending:originalInvoke(command,args));
  await mount();
  await confirmActivation();
  expect(invoke).toHaveBeenCalledWith('activate_steel_dataset',{datasetId:normalized.id,review:{sourceSha256:normalized.sourceSha256,updatedAt:normalized.updatedAt}});
  fireEvent.click(screen.getByRole('button',{name:/第二合成.csv/}));
  await act(async()=>{resolve({...normalized,mappingState:'ready'});});
  expect(screen.getByRole('heading',{name:'第二合成.csv'})).toBeVisible();
  expect(screen.getByLabelText('我已核对当前版本的列映射和单位')).not.toBeChecked();
  expect(screen.queryByText('此数据集已激活，可在预测和科研工作流中选择。')).toBeNull();
});
it('does not replace the selected dataset when an older normalization request returns late',async()=>{
  const second={...original,id:'second',sourceName:'另一原始合成.csv'};
  datasets=[original,second];
  let resolve!:(value:unknown)=>void;
  const pending=new Promise(r=>{resolve=r;});
  const originalInvoke=vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command,args)=>command==='normalize_material_dataset'?pending:originalInvoke(command,args));
  await mount();
  const mapping=await screen.findByLabelText('C 规范字段');
  await waitFor(()=>expect(within(mapping).getByRole('option',{name:/chem.c/})).toBeVisible());
  fireEvent.change(mapping,{target:{value:'chem.c'}});
  fireEvent.change(screen.getByLabelText('C 原始单位'),{target:{value:'wt%'}});
  await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'确认映射与单位，创建新版本'}));});
  fireEvent.click(screen.getByRole('button',{name:/另一原始合成.csv/}));
  await act(async()=>{resolve(normalized);});
  expect(screen.getByRole('heading',{name:'另一原始合成.csv'})).toBeVisible();
  expect(screen.getByLabelText('C 规范字段')).toHaveValue('');
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='activate_steel_dataset')).toBe(false);
});
it('refuses an activation reply for a different saved source instead of claiming success',async()=>{
  datasets=[normalized];
  const originalInvoke=vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command,args)=>command==='activate_steel_dataset'?
    Promise.resolve({...normalized,sourceSha256:'c'.repeat(64),mappingState:'ready'}):originalInvoke(command,args));
  await mount();
  await confirmActivation();
  expect(await screen.findByRole('alert')).toHaveTextContent('数据集版本发生变化');
  expect(screen.queryByText('此数据集已激活，可在预测和科研工作流中选择。')).toBeNull();
});
it('keeps the old ID-only desktop activation call compatible',async()=>{
  datasets=[normalized];
  await desktop.activateSteelDataset(normalized.id);
  expect(invoke).toHaveBeenLastCalledWith('activate_steel_dataset',{datasetId:normalized.id});
});
