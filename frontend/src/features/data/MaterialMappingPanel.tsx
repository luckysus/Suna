import { useEffect, useState } from 'react';
import { desktop, type SteelDatasetRecord, type MaterialRegistry } from '../../bridge/desktop';
import { useResearchStore } from '../../stores/researchStore';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
export default function MaterialMappingPanel({ dataset, onSaved }: {
    dataset: SteelDatasetRecord;
    onSaved: () => void;
}) {
    const project = useResearchStore(s => s.currentProject), [registry, setRegistry] = useState<MaterialRegistry | null>(null), [suggestions, setSuggestions] = useState<{
        fieldId: string;
        confidence: number;
        reason: string;
    }[][]>([]), [mapping, setMapping] = useState<Record<number, {
        fieldId: string;
        sourceUnit: string;
    }>>({}), [error, setError] = useState(''), [notice, setNotice] = useState(''), [overlay, setOverlay] = useState('');
    useEffect(() => { setMapping({}); setNotice(''); void Promise.all([desktop.getMaterialRegistry(project?.id), desktop.suggestMaterialFields(dataset.columns.map(c => c.originalName), project?.id)]).then(([r, s]) => { setRegistry(r); setSuggestions(s); }).catch(e => setError(String(e))); }, [dataset.id, project?.id]);
    return <section aria-label='材料字段映射'><h3>材料字段与单位规范化</h3>{error && <p role='alert'>{error}</p>}{notice && <p role='status'>{notice}</p>}<p>建议仅供审核；手动选择字段和源单位后创建新版本，原始数据保持完整。HV、HB、HRC 分别处理。</p>{dataset.columns.filter(c => c.inferredType === 'number').map(c => <div key={c.ordinal}><strong>{c.originalName}</strong><span>{suggestions[c.ordinal]?.map(s => `${s.fieldId} ${Math.round(s.confidence * 100)}% (${s.reason})`).join(' / ') || '无可靠别名匹配'}</span><label>{c.originalName} 规范字段<select value={mapping[c.ordinal]?.fieldId ?? ''} onChange={e => setMapping(a => ({ ...a, [c.ordinal]: { fieldId: e.target.value, sourceUnit: a[c.ordinal]?.sourceUnit ?? c.unit ?? '' } }))}><option value=''>不转换此列</option>{registry?.fields.filter(f => f.valueType === 'float' && f.role !== 'derived').map(f => <option key={f.id} value={f.id}>{f.id} · {f.role} · {f.canonicalUnit}</option>)}</select></label><label>{c.originalName} 原始单位<Input value={mapping[c.ordinal]?.sourceUnit ?? ''} onChange={e => setMapping(a => ({ ...a, [c.ordinal]: { fieldId: a[c.ordinal]?.fieldId ?? '', sourceUnit: e.target.value } }))}/></label></div>)}<Button disabled={!project || !Object.values(mapping).some(m => m.fieldId)} onClick={() => { if (!project)
        return; void desktop.normalizeMaterialDataset({ projectId: project.id, datasetId: dataset.id, mappings: Object.entries(mapping).filter(([, m]) => m.fieldId).map(([ordinal, m]) => ({ ordinal: Number(ordinal), ...m })) }).then(() => { setNotice('规范化新版本已保存，请确认并激活'); onSaved(); }).catch(e => setError(String(e))); }}>确认映射与单位，创建新版本</Button>
 <details><summary>项目自定义字段覆盖层</summary><p>支持 fields 数组与 overrides 数组；替换已有规范字段必须显式列入 overrides。所有修改保存新版本。</p><textarea aria-label='材料字段覆盖 JSON' value={overlay} onChange={e => setOverlay(e.target.value)}/><Button disabled={!project} onClick={() => { if (!project)
        return; try {
        const input = JSON.parse(overlay);
        void desktop.saveMaterialSchemaOverlay(project.id, input.fields, input.overrides ?? []).then(() => desktop.getMaterialRegistry(project.id)).then(setRegistry).catch(e => setError(String(e)));
    }
    catch (e) {
        setError(String(e));
    } }}>保存字段覆盖版本</Button></details></section>;
}
