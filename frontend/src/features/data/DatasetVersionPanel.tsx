import type {SteelDatasetRecord} from '../../bridge/desktop';
import DatasetActivationPanel from './DatasetActivationPanel';
import MaterialMappingPanel from './MaterialMappingPanel';

export default function DatasetVersionPanel({dataset, disabled, onSaved}: {
  dataset:SteelDatasetRecord;
  disabled:boolean;
  onSaved:(saved:SteelDatasetRecord)=>void;
}) {
  return <>
    <DatasetActivationPanel dataset={dataset} disabled={disabled} onActivated={onSaved}/>
    <MaterialMappingPanel dataset={dataset} onSaved={onSaved}/>
  </>;
}
