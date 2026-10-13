import { useEffect, useState } from 'react';
import { desktop, type SteelDatasetRecord, type MaterialVariable, type MaterialRegistry, type ExperimentDesignResult } from '../../bridge/desktop';
import { Button } from '../../components/ui/button';
import { Input } from '../../components/ui/input';
import { useComputeTask, ComputeTaskStatus } from '../tasks/useComputeTask';
export default function ResearchNextRoundPanel({ projectId }: {
    projectId: string;
}) {
    const [datasets, setDatasets] = useState<SteelDatasetRecord[]>([]), [datasetId, setDatasetId] = useState(''), [registry, setRegistry] = useState<MaterialRegistry | null>(null), [variables, setVariables] = useState<MaterialVariable[]>([]), [property, setProperty] = useState(''), [strategy, setStrategy] = useState<'exploitation' | 'exploration' | 'balanced'>('balanced'), [budget, setBudget] = useState(3), [direction, setDirection] = useState<'maximize' | 'minimize'>('maximize'), [error, setError] = useState(''), [result, setResult] = useState<ExperimentDesignResult | null>(null), [notice, setNotice] = useState('');
    const task = useComputeTask(`suna.feedback.${projectId}`, desktop.getComputeDesignResult), dataset = datasets.find(d => d.id === datasetId);
    const load = async () => { try {
        const [all, links, schema] = await Promise.all([desktop.listSteelDatasets(), desktop.listResearchProjectObjects(projectId, 100), desktop.getMaterialRegistry(projectId)]);
        setDatasets(all.filter(d => d.mappingState === 'ready' && links.some(o => o.kind === 'dataset' && o.id === d.id)));
        setRegistry(schema);
    }
    catch (e) {
        setError(String(e));
    } };
    useEffect(() => { void load(); }, [projectId]);
    useEffect(() => { if (task.result)
        setResult(task.result); }, [task.result]);
    useEffect(() => { setVariables([]); setProperty(''); setResult(null); }, [datasetId]);
    const add = (name: string) => { const c = dataset?.columns.find(c => (c.canonicalField || c.originalName) === name); if (!c || variables.some(v => v.name === name))
        return; setVariables(a => [...a, { name, fieldId: c.canonicalField?.includes('.') ? c.canonicalField : '', unit: c.unit ?? '', min: c.min ?? 0, max: c.max ?? 1 }]); };
    const run = async () => { try {
        const next = await desktop.recommendNextResearchExperiments({ projectId, datasetId, property, variables, strategy, budget, direction, seed: 42, minDistance: 0.01 });
        setResult(await task.run(next.id));
        setError('');
    }
    catch (e) {
        setError(String(e));
    } };
    return <section aria-label='下一轮实验推荐'><h3>基于审核实测推荐下一轮实验</h3><p>先创建包含已审核测量的新版本，确认映射并激活。GP 至少需要 5 行；预算最多 20。重复候选和硬约束不通过的候选不会进入推荐。</p>{error && <p role='alert'>{error}</p>}{notice && <p role='status'>{notice}</p>}<Button variant='ghost' onClick={() => void load()}>刷新新数据版本</Button><label>已激活的项目数据集<select value={datasetId} onChange={e => setDatasetId(e.target.value)}><option value=''>选择审核后的新版本</option>{datasets.map(d => <option key={d.id} value={d.id}>{d.sourceName}</option>)}</select></label><label>优化性能<select value={property} onChange={e => setProperty(e.target.value)}><option value=''>选择实测性能列</option>{dataset?.columns.filter(c => c.inferredType === 'number').map(c => <option key={c.ordinal} value={c.canonicalField || c.originalName}>{c.originalName} · {c.unit}</option>)}</select></label><label>增加实验变量<select value='' onChange={e => add(e.target.value)}><option value=''>选择参数列</option>{dataset?.columns.filter(c => c.inferredType === 'number' && (c.canonicalField || c.originalName) !== property && !variables.some(v => v.name === (c.canonicalField || c.originalName))).map(c => <option key={c.ordinal} value={c.canonicalField || c.originalName}>{c.originalName}</option>)}</select></label>{variables.map((v, i) => <div key={v.name}><strong>{v.name} · {v.unit}</strong><label>{v.name} 字段角色<select value={v.fieldId} onChange={e => setVariables(a => a.map((v, n) => n === i ? { ...v, fieldId: e.target.value } : v))}><option value=''>确认规范字段</option>{registry?.fields.filter(f => ['composition', 'process', 'microstructure'].includes(f.role)).map(f => <option key={f.id}>{f.id}</option>)}</select></label><label>{v.name} 推荐下限<Input type='number' value={v.min} onChange={e => setVariables(a => a.map((v, n) => n === i ? { ...v, min: Number(e.target.value) } : v))}/></label><label>{v.name} 推荐上限<Input type='number' value={v.max} onChange={e => setVariables(a => a.map((v, n) => n === i ? { ...v, max: Number(e.target.value) } : v))}/></label><Button variant='ghost' onClick={() => setVariables(a => a.filter((_, n) => n !== i))}>删除 {v.name}</Button></div>)}<label>策略<select value={strategy} onChange={e => setStrategy(e.target.value as typeof strategy)}><option value='balanced'>平衡性能与信息增益</option><option value='exploration'>探索不确定区域</option><option value='exploitation'>利用性能较好区域</option></select></label><label>方向<select value={direction} onChange={e => setDirection(e.target.value as typeof direction)}><option value='maximize'>最大化</option><option value='minimize'>最小化</option></select></label><label>实验预算<Input type='number' min={1} max={20} value={budget} onChange={e => setBudget(Number(e.target.value))}/></label><Button disabled={!datasetId || !property || !variables.length || variables.some(v => !v.fieldId || !v.unit) || task.task?.state === 'running'} onClick={() => void run()}>提交下一轮推荐</Button><ComputeTaskStatus {...task}/>{result && <><p>不确定性来源：基于当前实测数据拟合的 GP 代理预测标准差。</p><table><thead><tr><th>候选参数</th><th>预测均值</th><th>GP 标准差</th><th>候选操作</th></tr></thead><tbody>{result.points.map((point, i) => <tr key={i}><td>{Object.entries(point.values).map(([k, v]) => `${k}=${v.toFixed(4)}`).join(' / ')}</td><td>{point.predicted_mean ?? '未计算'}</td><td>{point.predicted_std ?? '未计算'}</td><td><Button onClick={() => { void desktop.createResearchDesignCandidatePlan(projectId, task.task?.id ?? '', i, `下一轮候选 ${i + 1}`).then(() => { setNotice('候选已加入实验计划，需人工确认'); window.dispatchEvent(new Event('suna:experiment-plans-updated')); }).catch(e => setError(String(e))); }}>加入待确认实验计划</Button></td></tr>)}</tbody></table></>}</section>;
}
