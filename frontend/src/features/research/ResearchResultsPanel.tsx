import ResearchPackagePanel from "./package/ResearchPackagePanel";
import {packageService} from "./package/desktopService";
import { useEffect, useState, useRef } from 'react';
import { desktop, type ResearchArtifact, type ResearchRun, type ResearchArtifactEdge } from '../../bridge/desktop';
import {buildReportDocx,buildReportPptx,saveReportBlob,printReport} from '../reports/reportExport';
import { Button } from '../../components/ui/button';
export default function ResearchResultsPanel({ projectId }: {
    projectId: string;
}) {
    const [runs, setRuns] = useState<ResearchRun[]>([]), [artifacts, setArtifacts] = useState<ResearchArtifact[]>([]), [edges, setEdges] = useState<ResearchArtifactEdge[]>([]), [detail, setDetail] = useState<ResearchArtifact | null>(null), [offset, setOffset] = useState(0), [error, setError] = useState(''),[report,setReport]=useState<Awaited<ReturnType<typeof desktop.generateResearchProjectReport>>|null>(null),[notice,setNotice]=useState('');
    const request=useRef(0);
    const [edgeOffset,setEdgeOffset]=useState(0);
    const load = async () => { const version=++request.current;try {
        const [r,a,e] = await Promise.all([desktop.listResearchRuns(projectId,25,offset), desktop.listResearchArtifacts(projectId,25,offset),desktop.listResearchArtifactEdges(projectId,100,edgeOffset)]);
        if(version!==request.current)return;setRuns(r);setArtifacts(a);setEdges(e);setError('');
    }catch(e){if(version===request.current)setError(String(e));}};
    useEffect(()=>{setOffset(0);setEdgeOffset(0);setDetail(null);setReport(null);setNotice('');},[projectId]);
    useEffect(()=>{void load();let active=true;let cleanup:(()=>void)|undefined;let timer:ReturnType<typeof setTimeout>|undefined;
      void desktop.listenSchedulerProgress(()=>{if(active){clearTimeout(timer);timer=setTimeout(()=>{if(active)void load();},250);}}).then(c=>{if(active)cleanup=c;else c();});
      return()=>{active=false;request.current++;clearTimeout(timer);cleanup?.();};
    },[projectId,offset,edgeOffset]);
    const show = async (id: string) => { try {
        setDetail(await desktop.getResearchArtifact(id));
    }
    catch (e) {
        setError(String(e));
    } };
    const perform=async(f:()=>Promise<unknown>)=>{try{await f();setError('');}catch(e){setError(String(e));}};
    const openTask=async(id:string)=>{const n=await desktop.getResearchRunNavigation(id);const section=n.kind==='optimization'?'optimization':n.kind==='training'||n.kind==='prediction'?'prediction':n.kind==='analysis'?'data':n.kind==='literature'?'literature':'experiment';window.history.pushState(null,'',`/${section}?project=${encodeURIComponent(n.projectId)}&task=${encodeURIComponent(id)}`);window.dispatchEvent(new PopStateEvent('popstate'));};
    const openChat=async(id:string)=>{const n=await desktop.getResearchRunNavigation(id);if(!n.conversationId)throw new Error('此任务没有 Agent 会话来源');window.history.pushState(null,'',`/chat?conversation=${encodeURIComponent(n.conversationId)}&project=${encodeURIComponent(projectId)}`);window.dispatchEvent(new PopStateEvent('popstate'));};
    return <section aria-label="科研成果与溯源"><h3>活动与成果</h3><ResearchPackagePanel key={projectId} projectId={projectId} service={packageService}/>{notice&&<p role="status">{notice}</p>}<Button onClick={()=>void perform(async()=>{const m=await desktop.exportResearchManifest(projectId);const result=await desktop.saveExportFile('research_manifest.json',Array.from(new TextEncoder().encode(JSON.stringify(m,null,2))));setNotice(result?'项目 manifest 已导出':'导出已取消');})}>导出可复核 manifest</Button><Button onClick={()=>void perform(async()=>{setReport(await desktop.generateResearchProjectReport(projectId));setNotice('项目报告已持久化');})}>生成项目成果报告</Button>{report&&<details open><summary>{report.title}</summary><pre>{report.content_markdown}</pre>{(['markdown','docx','pptx','pdf'] as const).map(format=><Button key={format} onClick={()=>void perform(async()=>{if(format==='pdf'){printReport(report.content_markdown,report.title);return;}const blob=format==='docx'?await buildReportDocx(report.content_markdown,report.title):format==='pptx'?await buildReportPptx(report.content_markdown,report.title):new Blob([report.content_markdown],{type:'text/markdown'});setNotice(await saveReportBlob(blob,report.title,format==='markdown'?'md':format,format)?'报告已导出':'导出已取消');})}>{format.toUpperCase()}</Button>)}</details>}{error && <p role="alert">{error}</p>}<Button onClick={() => void load()}>对账并刷新成果</Button><ol>{runs.map(r => <li key={r.id}><strong>{r.kind} · {r.state} · {r.progress}%</strong><span>{r.origin} · 原任务 {r.backgroundTaskId ?? '同步活动'} · {r.createdAt}</span>{r.errorCode && <p>{r.errorCode}</p>}{r.backgroundTaskId&&<Button variant="ghost" onClick={()=>void perform(()=>openTask(r.backgroundTaskId!))}>打开专业页面原任务</Button>}{r.agentRunId&&r.backgroundTaskId&&<Button variant="ghost" onClick={()=>void perform(()=>openChat(r.backgroundTaskId!))}>返回来源会话</Button>}</li>)}</ol>{!runs.length && <p>当前项目暂无计算活动。选定当前项目后提交训练或优化，任务会自动归入项目。</p>}<ul>{artifacts.map(a => <li key={a.id}><Button variant="ghost" onClick={() => void show(a.id)}>{a.title} · {a.kind}</Button><small>SHA256 {a.contentSha256}</small></li>)}</ul><h4>成果来源</h4><Button disabled={!edgeOffset} onClick={()=>setEdgeOffset(Math.max(0,edgeOffset-100))}>前一页关系</Button><Button disabled={edges.length<100} onClick={()=>setEdgeOffset(edgeOffset+100)}>更多来源关系</Button><ul>{edges.map(e => <li key={`${e.fromId}:${e.toId}:${e.relation}`}><Button variant="ghost" onClick={() => void show(e.fromId)}>{e.fromId}</Button> → {e.relation} → <Button variant="ghost" onClick={() => void show(e.toId)}>{e.toId}</Button></li>)}</ul>{detail && <details open><summary>{detail.title} · {detail.schemaVersion}</summary><pre>{JSON.stringify(detail.payload, null, 2)}</pre></details>}<Button disabled={!offset} onClick={() => setOffset(Math.max(0, offset - 25))}>上一页成果</Button><Button disabled={runs.length < 25 && artifacts.length < 25} onClick={() => setOffset(offset + 25)}>下一页成果</Button></section>;
}
