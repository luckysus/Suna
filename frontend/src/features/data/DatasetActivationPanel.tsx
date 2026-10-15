import {useEffect, useRef, useState} from 'react';
import {desktop, type SteelDatasetRecord} from '../../bridge/desktop';
import {Button} from '../../components/ui/button';
import {errorMessage} from '../../utils/errorMessage';

export default function DatasetActivationPanel({dataset, onActivated, disabled=false}: {
  dataset:SteelDatasetRecord;
  onActivated:(dataset:SteelDatasetRecord)=>void;
  disabled?:boolean;
}) {
  const [confirmed,setConfirmed]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('');
  const epoch=useRef(0);
  useEffect(()=>{
    epoch.current++;setConfirmed(false);setBusy(false);setError('');
    return()=>{epoch.current++;};
  },[dataset.id,dataset.sourceSha256,dataset.updatedAt]);
  const mapped=dataset.columns.some(c=>c.canonicalField?.trim());
  const duplicates=dataset.columns.some(c=>c.duplicate);
  const activate=async()=>{
    if(!confirmed||busy||disabled||!mapped||duplicates||dataset.mappingState==='ready')return;
    const token=epoch.current;setBusy(true);setError('');
    try {
      const saved=await desktop.activateSteelDataset(dataset.id,{sourceSha256:dataset.sourceSha256,updatedAt:dataset.updatedAt});
      if(token!==epoch.current)return;
      if(saved.id!==dataset.id||saved.sourceSha256!==dataset.sourceSha256||saved.mappingState!=='ready')throw new Error('数据集版本发生变化，请刷新后重新核对。');
      onActivated(saved);
    } catch(reason) {
      if(token===epoch.current){
        const message=errorMessage(reason,'无法激活此数据集');
        setError(message==='dataset_activation_review_changed'?'数据集版本发生变化，请刷新后重新核对。':message);
      }
    } finally {
      if(token===epoch.current)setBusy(false);
    }
  };
  return <section aria-label="数据集激活"><h3>核对已保存的数据版本</h3>
    <p style={{overflowWrap:'anywhere'}}>来源 SHA256：{dataset.sourceSha256}</p>
    <div style={{maxHeight:240,overflow:'auto'}}><table><thead><tr><th>原列</th><th>已保存规范字段</th><th>已保存单位</th></tr></thead>
      <tbody>{dataset.columns.map(c=><tr key={c.ordinal}><td>{c.originalName}</td><td>{c.canonicalField||'未映射'}</td><td>{c.unit||'未记录'}</td></tr>)}</tbody></table></div>
    {error&&<p role="alert">{error}</p>}
    {dataset.mappingState==='ready'?<p role="status">此数据集已激活，可在预测和科研工作流中选择。</p>:<>
      <p>核对后显式激活此版本。未映射列保留原名，单位保持未记录；训练和优化仍需通过各自校验。</p>
      {!mapped&&<p>请先在下方确认字段和单位，创建规范化新版本。</p>}
      {duplicates&&<p>存在重复列，请先处理后再激活。</p>}
      <label><input type="checkbox" checked={confirmed} disabled={busy||disabled||!mapped||duplicates} onChange={e=>setConfirmed(e.target.checked)}/>我已核对当前版本的列映射和单位</label>
      <Button disabled={busy||disabled||!confirmed||!mapped||duplicates} onClick={()=>void activate()}>{busy?'正在核验并激活…':'确认激活此数据集'}</Button>
    </>}
  </section>;
}
