import { useEffect, useState } from "react";
import { desktop, type ResearchProject } from "../../bridge/desktop";
import { useResearchStore } from "../../stores/researchStore";
export default function ResearchProjectContext() {
  const { currentProject, setProject } = useResearchStore();
  const [projects,setProjects]=useState<ResearchProject[]>([]);
  const [error,setError]=useState("");
  const load=async()=>{try{const [list,current]=await Promise.all([desktop.listResearchProjects({status:"active",limit:100}),desktop.getCurrentResearchProject()]);setProjects(list);setProject(current);}catch(e){setError(e instanceof Error?e.message:String(e));}};
  useEffect(()=>{void load();const refresh=()=>{void load();};window.addEventListener("suna:projects-updated",refresh);return()=>window.removeEventListener("suna:projects-updated",refresh);},[]);
  const select=async(id:string)=>{try{await desktop.setCurrentResearchProject(id||null);setProject(projects.find((p)=>p.id===id)??null);setError("");}catch(e){setError(String(e));}};
  return <div className="suna-project-context"><label>当前科研项目<select aria-label="当前科研项目" value={currentProject?.id??""} onChange={(e)=>void select(e.target.value)}><option value="">未指定项目</option>{projects.map((p)=><option key={p.id} value={p.id}>{p.name}</option>)}{currentProject?.status==="archived"&&<option value={currentProject.id}>{currentProject.name}（已归档）</option>}</select></label>{error&&<span role="status">项目上下文暂不可用</span>}</div>;
}
