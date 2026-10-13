import {render,screen,fireEvent,waitFor} from '@testing-library/react';
import {expect,it,vi,beforeEach} from 'vitest';
import MaterialConstraintsPanel from './MaterialConstraintsPanel';
import {desktop} from '../../bridge/desktop';
import {useResearchStore} from '../../stores/researchStore';
vi.mock('../../bridge/desktop',()=>({desktop:{getMaterialRegistry:vi.fn(),listMaterialRules:vi.fn(),saveMaterialRule:vi.fn(),previewMaterialRules:vi.fn()}}));
beforeEach(()=>{useResearchStore.setState({currentProject:{id:'p',name:'课题',description:'',materialSystem:'steel',status:'active',tags:[],createdAt:'now',updatedAt:'now'}});vi.mocked(desktop.getMaterialRegistry).mockResolvedValue({schemaVersion:'v1',materialSystem:'steel',fields:[],templates:[]});vi.mocked(desktop.listMaterialRules).mockResolvedValue([{version:2,rule:{id:'rule',name:'设备规则',severity:'hard',enabled:true,penalty:1,provenance:{type:'user',sourceRef:'device',sourceVersion:'1',reviewed:true},expression:{kind:'range',fieldId:'process.t',unit:'degC',min:800}}}]);});
it('persists disabling as a new revision',async()=>{render(<MaterialConstraintsPanel variables={[]} onChange={()=>{}}/>);fireEvent.click(await screen.findByRole('checkbox',{name:/设备规则/}));await waitFor(()=>expect(desktop.saveMaterialRule).toHaveBeenCalledWith('p',expect.objectContaining({enabled:false}),2));});
it('shows conflict rejection from the real compile API',async()=>{vi.mocked(desktop.previewMaterialRules).mockRejectedValue(new Error('hard_rule_conflict'));render(<MaterialConstraintsPanel variables={[]} onChange={()=>{}}/>);await screen.findByRole('checkbox',{name:/设备规则/});fireEvent.click(screen.getByRole('button',{name:'检查生效规则与冲突'}));expect(await screen.findByRole('alert')).toHaveTextContent('hard_rule_conflict');});
