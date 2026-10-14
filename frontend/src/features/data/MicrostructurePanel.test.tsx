import {render,screen,fireEvent,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import MicrostructurePanel,{imagePoint} from './MicrostructurePanel';
import {desktop} from '../../bridge/desktop';
import {useResearchStore} from '../../stores/researchStore';
vi.mock('../../bridge/desktop',()=>({desktop:{listMicrostructureImages:vi.fn(),getMicrostructurePreview:vi.fn(),getMicrostructureAnnotation:vi.fn(),listMicrostructureAnalyses:vi.fn(),saveMicrostructureAnnotation:vi.fn(),reviewMicrostructureAnalysis:vi.fn(),getMicrostructureTaskResult:vi.fn(),openFileDialog:vi.fn(),importMicrostructureImage:vi.fn()}}));
const image={id:'i',projectId:'p',sampleId:'合成样本',modality:'OM',sourceType:'synthetic',sourceSha256:'sha',width:200,height:100,metadata:{},createdAt:'now'};
beforeEach(()=>{vi.clearAllMocks();localStorage.clear();useResearchStore.setState({currentProject:{id:'p',name:'课题',description:'',materialSystem:null,status:'active',tags:[],createdAt:'now',updatedAt:'now'}});vi.mocked(desktop.listMicrostructureImages).mockResolvedValue([image]);vi.mocked(desktop.getMicrostructurePreview).mockResolvedValue([]);vi.mocked(desktop.getMicrostructureAnnotation).mockResolvedValue(null);vi.mocked(desktop.listMicrostructureAnalyses).mockResolvedValue([]);vi.stubGlobal('URL',Object.assign(URL,{createObjectURL:vi.fn(()=> 'blob:preview'),revokeObjectURL:vi.fn()}));});
it('converts client coordinates independently of DPI and CSS scale',()=>{expect(imagePoint(110,70,{left:10,top:20,width:400,height:200},800,400)).toEqual([200,100]);expect(imagePoint(60,45,{left:10,top:20,width:200,height:100},800,400)).toEqual([200,100]);});
it('saves pixel scale from keyboard points and real distance',async()=>{vi.mocked(desktop.saveMicrostructureAnnotation).mockResolvedValue({id:'a',imageId:'i',parameters:{calibration:{points:[[0,0],[20,0]],distance:100,unit:'um'},scaleMicronsPerPixel:5,rois:[],threshold:128,maskConfirmed:false},sha256:'hash'});render(<MicrostructurePanel/>);await screen.findByRole('button',{name:/合成样本 · OM/});await screen.findByRole('button',{name:'添加坐标点'});fireEvent.click(screen.getByRole('button',{name:'添加坐标点'}));fireEvent.change(screen.getByLabelText('像素 X'),{target:{value:'20'}});fireEvent.click(screen.getByRole('button',{name:'添加坐标点'}));fireEvent.click(screen.getByRole('button',{name:'保存标注版本'}));await waitFor(()=>expect(desktop.saveMicrostructureAnnotation).toHaveBeenCalledWith('i',expect.objectContaining({calibration:{points:[[0,0],[20,0]],distance:100,unit:'um'}})));expect(await screen.findByText('标注已保存；5 μm/px')).toBeInTheDocument();});
it('keeps uncalibrated results in pixels and sends a persisted review',async()=>{vi.mocked(desktop.listMicrostructureAnalyses).mockResolvedValue([{id:'a',taskId:'t',annotationId:'v',result:{algorithm:'microstructure.v1',sourceSha256:'sha',scaleMicronsPerPixel:null,threshold:128,histogram:Array(256).fill(0),metrics:{area:{value:10,unit:'px²',qualifiedName:'ROI像素面积'}},regions:[],reviewed:false,limitations:['连通区域不是晶粒']},error:null,reviewState:null,reviewNote:null,createdAt:'now'}]);render(<MicrostructurePanel/>);expect(await screen.findByText(/未标定（像素）/)).toBeInTheDocument();expect(screen.getByText('px²')).toBeInTheDocument();fireEvent.click(screen.getByRole('button',{name:'接受统计结果'}));await waitFor(()=>expect(desktop.reviewMicrostructureAnalysis).toHaveBeenCalledWith('a','accepted',''));});
it('requires a project instead of inventing a source',()=>{useResearchStore.setState({currentProject:null});render(<MicrostructurePanel/>);expect(screen.getByText(/先选择当前科研项目/)).toBeInTheDocument();});
it('imports a real selected path with explicit 16bit window parameters',async()=>{
 vi.mocked(desktop.openFileDialog).mockResolvedValue('C:\\授权图像\\样本.tif');
 vi.mocked(desktop.importMicrostructureImage).mockResolvedValue({...image,metadata:{rasterInput:{version:'microstructure-input.v1',format:'TIFF',decodedBitDepth:16,colorModel:'gray',width:200,height:100,sourceExifOrientation:1,orientationPolicy:'ignore-exif',coordinateSystem:'raw-raster',alphaPolicy:'white-composite',iccPolicy:'unmanaged',grayscaleMapping:{version:'linear-u16.v1',mode:'window',lower:1000,upper:5000,rounding:'floor'}}}});
 render(<MicrostructurePanel/>);
 fireEvent.change(screen.getByLabelText('样本 ID'),{target:{value:'新样本'}});
 fireEvent.change(screen.getByLabelText('16 位灰度映射'),{target:{value:'window'}});
 fireEvent.change(screen.getByLabelText('灰度窗口下限'),{target:{value:'1000'}});
 fireEvent.change(screen.getByLabelText('灰度窗口上限'),{target:{value:'5000'}});
 fireEvent.click(screen.getByRole('button',{name:'导入图像'}));
 await waitFor(()=>expect(desktop.importMicrostructureImage).toHaveBeenCalledWith(expect.objectContaining({sourcePath:'C:\\授权图像\\样本.tif',gray16Mapping:{mode:'window',lower:1000,upper:5000}})));
 expect(desktop.openFileDialog).toHaveBeenCalledWith(expect.objectContaining({filters:[{name:'显微图像',extensions:['png','jpg','jpeg','tif','tiff']}]}));
});
it('rejects an invalid intensity window before opening or importing a file',async()=>{
 render(<MicrostructurePanel/>);fireEvent.change(screen.getByLabelText('样本 ID'),{target:{value:'s'}});
 fireEvent.change(screen.getByLabelText('16 位灰度映射'),{target:{value:'window'}});
 fireEvent.change(screen.getByLabelText('灰度窗口下限'),{target:{value:'65535'}});
 fireEvent.change(screen.getByLabelText('灰度窗口上限'),{target:{value:'4'}});
 fireEvent.click(screen.getByRole('button',{name:'导入图像'}));
 expect(await screen.findByRole('alert')).toHaveTextContent('16 位灰度窗口须为');
 expect(desktop.openFileDialog).not.toHaveBeenCalled();expect(desktop.importMicrostructureImage).not.toHaveBeenCalled();
});
it('shows the persisted mapping rather than applying guessed exposure or magnification',async()=>{
 vi.mocked(desktop.listMicrostructureImages).mockResolvedValue([{...image,metadata:{rasterInput:{version:'microstructure-input.v1',format:'TIFF',decodedBitDepth:16,colorModel:'gray',width:200,height:100,sourceExifOrientation:1,orientationPolicy:'ignore-exif',coordinateSystem:'raw-raster',alphaPolicy:'white-composite',iccPolicy:'unmanaged',grayscaleMapping:{version:'linear-u16.v1',mode:'window',lower:1000,upper:5000,rounding:'floor'}}}}]);
 render(<MicrostructurePanel/>);
 expect(await screen.findByText(/灰度映射：1000–5000 → 0–255/)).toHaveTextContent('原始栅格坐标');
});
