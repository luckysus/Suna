import { useEffect, useState, useRef } from "react";
import { desktop, type ResearchProject, type ResearchObject, type ResearchObjectKind, type ResearchSummary } from "../../bridge/desktop";
import { Button } from "../../components/ui/button";
import { Dialog,DialogContent,DialogTitle,DialogDescription } from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { useResearchStore } from "../../stores/researchStore";
import "./research-projects.css";
import ResearchMeasurementsPanel from "./ResearchMeasurementsPanel";
import ResearchResultsPanel from "./ResearchResultsPanel";
const labels: Record<string, string> = { dataset: "数据集", model: "模型", background_task: "后台任务", conversation: "会话", knowledge_base: "知识库", document: "文献", agent_run: "Agent 运行", optimization: "优化结果", experiment: "实验计划", report: "报告", artifact: "成果" };
const tabs = ["总览", "数据与知识", "模型与优化", "实验", "成果与报告", "活动与溯源"];
export default function ResearchProjectsPage() {
    const [projects, setProjects] = useState<ResearchProject[]>([]), [selected, setSelected] = useState<ResearchProject | null>(null), [search, setSearch] = useState(""), [status, setStatus] = useState("active"), [offset, setOffset] = useState(0), [loading, setLoading] = useState(false), [error, setError] = useState("");
    const [editing, setEditing] = useState<ResearchProject | "new" | null>(null), [name, setName] = useState(""), [description, setDescription] = useState(""), [material, setMaterial] = useState(""), [tags, setTags] = useState("");
    const [objects, setObjects] = useState<ResearchObject[]>([]), [summary, setSummary] = useState<ResearchSummary | null>(null), [tab, setTab] = useState("总览"), [kind, setKind] = useState<ResearchObjectKind>("dataset"), [available, setAvailable] = useState<ResearchObject[]>([]), [target, setTarget] = useState(""), [objectOffset, setObjectOffset] = useState(0);
    const [attachSearch,setAttachSearch]=useState(""),[attachOffset,setAttachOffset]=useState(0);
    const listRequest=useRef(0),detailRequest=useRef(0),openRequest=useRef(0);
    const setProject = useResearchStore((s) => s.setProject);
    const refresh = async () => { const request=++listRequest.current;setLoading(true); try {
        const items=await desktop.listResearchProjects({ search, status: status || undefined, limit: 25, offset });
        if(request!==listRequest.current)return;setProjects(items);setError("");
    }
    catch (e) {
        if(request===listRequest.current)setError(String(e));
    }
    finally {
        if(request===listRequest.current)setLoading(false);
    } };
    useEffect(() => { void refresh(); return()=>{listRequest.current++;}; }, [search, status, offset]);
    const open = async (id: string) => { const request=++openRequest.current;detailRequest.current++;setObjects([]);setSummary(null);setLoading(true); try {
        const p = await desktop.getResearchProject(id);
        if(request!==openRequest.current)return;setSelected(p);
        setObjectOffset(0);
        setError("");
    }
    catch (e) {
        if(request===openRequest.current)setError(String(e));
    }
    finally {
        if(request===openRequest.current)setLoading(false);
    } };
    useEffect(() => { const id = new URLSearchParams(window.location.search).get("project"); if (id)
        void open(id); }, []);
    const loadDetails = async () => { if (!selected)
        return; const request=++detailRequest.current;try {
        const [items, s] = await Promise.all([desktop.listResearchProjectObjects(selected.id, 50, objectOffset), desktop.getResearchProjectSummary(selected.id)]);
        if(request!==detailRequest.current)return;setObjects(items);
        setSummary(s);
    }
    catch (e) {
        setError(String(e));
    } };
    useEffect(() => { void loadDetails(); return()=>{detailRequest.current++;}; }, [selected?.id, objectOffset]);
    useEffect(() => { if (!selected)
        return; let active = true; void desktop.listResearchAttachableObjects(kind, attachSearch, 25,attachOffset).then((items) => { if (active) {
        setAvailable(items);
        setTarget(items[0]?.id ?? "");
    } }).catch((e) => { if (active)
        setError(String(e)); }); return () => { active = false; }; }, [selected?.id, kind,attachSearch,attachOffset]);
    const edit = (p: ResearchProject | "new") => { setEditing(p); setName(p === "new" ? "" : p.name); setDescription(p === "new" ? "" : p.description); setMaterial(p === "new" ? "" : p.materialSystem ?? ""); setTags(p === "new" ? "" : p.tags.join(", ")); };
    const save = async () => { try {
        const p = await desktop.saveResearchProject({ ...(editing && editing !== "new" ? { id: editing.id, expectedUpdatedAt: editing.updatedAt } : {}), name, description, materialSystem: material || null, tags: tags.split(/[,，]/).map((s) => s.trim()).filter(Boolean) });
        setEditing(null);
        setSelected(p);
        await refresh();
        window.dispatchEvent(new Event("suna:projects-updated"));
    }
    catch (e) {
        setError(String(e));
    } };
    const action = async (fn: () => Promise<unknown>) => { try {
        await fn();
        await loadDetails();
        await refresh();
        setError("");
    }
    catch (e) {
        setError(String(e));
    } };
    const visible = objects.filter((o) => tab === "总览" || tab === "活动与溯源" || (tab === "数据与知识" && ["dataset", "document", "knowledge_base"].includes(o.kind)) || (tab === "模型与优化" && ["model", "optimization", "background_task"].includes(o.kind)) || (tab === "实验" && o.kind === "experiment") || (tab === "成果与报告" && ["report", "artifact"].includes(o.kind)));
    return <section className="suna-projects-page"><header><div><h1>科研项目</h1><p>项目关联已有对象，归档与解绑保留原始数据。</p></div><Button onClick={() => edit("new")}>新建科研项目</Button></header>{error && <p role="alert">{error}</p>}{loading && <p role="status">正在读取科研项目…</p>}
 <div className="suna-projects-filters"><Input aria-label="搜索科研项目" value={search} onChange={(e) => { setSearch(e.target.value); setOffset(0); }} placeholder="搜索名称、目标或标签"/><select aria-label="项目状态" value={status} onChange={(e) => { setStatus(e.target.value); setOffset(0); }}><option value="active">活跃</option><option value="archived">已归档</option><option value="">全部</option></select></div>
 <div className="suna-projects-layout"><aside>{projects.map((p) => <button key={p.id} className={selected?.id === p.id ? "is-active" : ""} onClick={() => void open(p.id)}><strong>{p.name}</strong><span>{p.materialSystem || "材料体系未指定"} · {p.status === "archived" ? "已归档" : "活跃"}</span><small>{p.tags.join(" · ")}</small></button>)}{!loading && !projects.length && <p>暂无科研项目，创建项目后关联已有数据和任务。</p>}<Button disabled={!offset} onClick={() => setOffset(Math.max(0, offset - 25))}>上一页</Button><Button disabled={projects.length < 25} onClick={() => setOffset(offset + 25)}>下一页</Button></aside>
 {selected ? <article><header><h2>{selected.name}</h2><div><Button disabled={selected.status === "archived"} onClick={() => edit(selected)}>编辑项目</Button><Button onClick={() => void action(async () => { const p = await desktop.archiveResearchProject(selected.id, selected.status !== "archived"); setSelected(p); window.dispatchEvent(new Event("suna:projects-updated")); })}>{selected.status === "archived" ? "恢复项目" : "归档项目"}</Button><Button disabled={selected.status === "archived"} onClick={() => void action(async () => { await desktop.setCurrentResearchProject(selected.id); setProject(selected); window.dispatchEvent(new Event("suna:projects-updated")); })}>设为当前项目</Button></div></header><p>{selected.description || "未填写科研目标"}</p><nav aria-label="科研项目详情">{tabs.map((t) => <Button key={t} variant={t === tab ? "primary" : "ghost"} onClick={() => setTab(t)}>{t}</Button>)}</nav>
 {summary && <p>{Object.entries(summary.counts).map(([k, n]) => `${labels[k] ?? k} ${n}`).join(" · ") || "尚未关联对象"}{summary.missingSources > 0 ? ` · ${summary.missingSources} 个来源缺失` : ""}</p>}
 <fieldset disabled={selected.status === "archived"}><legend>关联已有对象</legend><select aria-label="关联对象类型" value={kind} onChange={(e) => {setKind(e.target.value as ResearchObjectKind);setAttachOffset(0);}}>{Object.entries(labels).filter(([k]) => k !== "artifact").map(([k, v]) => <option key={k} value={k}>{v}</option>)}</select><Input aria-label="搜索可关联对象" placeholder="搜索已有对象" value={attachSearch} onChange={e=>{setAttachSearch(e.target.value);setAttachOffset(0);}}/><Button variant="ghost" disabled={!attachOffset} onClick={()=>setAttachOffset(Math.max(0,attachOffset-25))}>上一页可关联对象</Button><Button variant="ghost" disabled={available.length<25} onClick={()=>setAttachOffset(attachOffset+25)}>下一页可关联对象</Button><select aria-label="关联已有对象" value={target} onChange={(e) => setTarget(e.target.value)}><option value="">选择当前工作区对象</option>{available.map((o) => <option key={o.id} value={o.id}>{o.title}</option>)}</select><Button disabled={!target} onClick={() => void action(() => desktop.attachResearchObject({ projectId: selected.id, kind, id: target }))}>关联到项目</Button></fieldset>
 {tab === "实验" && <ResearchMeasurementsPanel projectId={selected.id}/>}
 {(tab === "活动与溯源" || tab === "成果与报告") && <ResearchResultsPanel projectId={selected.id}/>}
 <ul>{visible.map((o) => <li key={`${o.kind}:${o.id}:${o.relation}`}><strong>{labels[o.kind]} · {o.title}</strong><span>{o.availability === "missing" ? "来源已删除或不可用" : o.relation}</span><Button disabled={selected.status === "archived"} variant="ghost" onClick={() => void action(() => desktop.detachResearchObject({ projectId: selected.id, kind: o.kind, id: o.id, relation: o.relation }))}>解除关联</Button></li>)}</ul>{!visible.length && <p>此视图没有已关联对象。</p>}<Button disabled={!objectOffset} onClick={() => setObjectOffset(Math.max(0, objectOffset - 50))}>前一页对象</Button><Button disabled={objects.length < 50} onClick={() => setObjectOffset(objectOffset + 50)}>更多对象</Button></article> : <article><h2>选择或创建项目</h2><p>从现有数据集、模型和任务开始组织课题。</p></article>}</div>
 <Dialog open={editing!==null} onOpenChange={open=>{if(!open)setEditing(null);}}><DialogContent className="suna-project-editor"><DialogTitle>{editing === "new" ? "新建科研项目" : "编辑科研项目"}</DialogTitle><DialogDescription>填写科研目标，原始数据和模型通过关联添加。</DialogDescription><label>项目名称<Input value={name} onChange={(e) => setName(e.target.value)} maxLength={160}/></label><label>科研目标与备注<textarea value={description} onChange={(e) => setDescription(e.target.value)}/></label><label>材料体系<Input value={material} onChange={(e) => setMaterial(e.target.value)}/></label><label>标签（逗号分隔）<Input value={tags} onChange={(e) => setTags(e.target.value)}/></label><Button disabled={!name.trim()} onClick={() => void save()}>保存项目</Button><Button variant="ghost" onClick={() => setEditing(null)}>取消</Button></DialogContent></Dialog>
 </section>;
}
