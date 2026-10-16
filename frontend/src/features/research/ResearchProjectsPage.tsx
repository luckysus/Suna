import { useEffect, useState, useRef } from "react";
import { Plus, Search } from "lucide-react";
import {
  desktop,
  type ResearchProject,
  type ResearchObject,
  type ResearchObjectKind,
  type ResearchSummary,
} from "../../bridge/desktop";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useResearchStore } from "../../stores/researchStore";
import "./research-projects.css";
import ResearchWorkflowGuide from "./ResearchWorkflowGuide";
import ResearchProjectWorkspace from "./ResearchProjectWorkspace";
import ResearchProjectListSidebar from "./ResearchProjectListSidebar";
import ResearchProjectEditorDialog from "./ResearchProjectEditorDialog";

const labels: Record<string, string> = {
  dataset: "数据集",
  model: "模型",
  background_task: "后台任务",
  conversation: "会话",
  knowledge_base: "知识库",
  document: "文献",
  agent_run: "Agent 运行",
  optimization: "优化结果",
  experiment: "实验计划",
  report: "报告",
  artifact: "成果",
};
const tabs = ["总览", "数据与知识", "模型与优化", "科研工作流", "实验", "成果与报告", "活动与溯源"];

export default function ResearchProjectsPage() {
  const [projects, setProjects] = useState<ResearchProject[]>([]);
  const [selected, setSelected] = useState<ResearchProject | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("active");
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [editing, setEditing] = useState<ResearchProject | "new" | null>(null);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [material, setMaterial] = useState("");
  const [tags, setTags] = useState("");
  const [objects, setObjects] = useState<ResearchObject[]>([]);
  const [summary, setSummary] = useState<ResearchSummary | null>(null);
  const [tab, setTab] = useState("总览");
  const [kind, setKind] = useState<ResearchObjectKind>("dataset");
  const [available, setAvailable] = useState<ResearchObject[]>([]);
  const [target, setTarget] = useState("");
  const [objectOffset, setObjectOffset] = useState(0);
  const [attachSearch, setAttachSearch] = useState("");
  const [attachOffset, setAttachOffset] = useState(0);

  const listRequest = useRef(0);
  const detailRequest = useRef(0);
  const openRequest = useRef(0);
  const setProject = useResearchStore((s) => s.setProject);

  const refresh = async () => {
    const request = ++listRequest.current;
    setLoading(true);
    try {
      const items = await desktop.listResearchProjects({ search, status: status || undefined, limit: 25, offset });
      if (request !== listRequest.current) return;
      setProjects(items);
      setError("");
    } catch (e) {
      if (request === listRequest.current) setError(String(e));
    } finally {
      if (request === listRequest.current) setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
    return () => { listRequest.current++; };
  }, [search, status, offset]);

  const open = async (id: string) => {
    const request = ++openRequest.current;
    detailRequest.current++;
    setObjects([]);
    setSummary(null);
    setLoading(true);
    try {
      const p = await desktop.getResearchProject(id);
      if (request !== openRequest.current) return;
      setSelected(p);
      setObjectOffset(0);
      setError("");
    } catch (e) {
      if (request === openRequest.current) setError(String(e));
    } finally {
      if (request === openRequest.current) setLoading(false);
    }
  };

  useEffect(() => {
    const id = new URLSearchParams(window.location.search).get("project");
    if (id) void open(id);
  }, []);

  const loadDetails = async () => {
    if (!selected) return;
    const request = ++detailRequest.current;
    try {
      const [items, s] = await Promise.all([
        desktop.listResearchProjectObjects(selected.id, 50, objectOffset),
        desktop.getResearchProjectSummary(selected.id),
      ]);
      if (request !== detailRequest.current) return;
      setObjects(items);
      setSummary(s);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    void loadDetails();
    return () => { detailRequest.current++; };
  }, [selected?.id, objectOffset]);

  useEffect(() => {
    if (!selected) return;
    let active = true;
    void desktop.listResearchAttachableObjects(kind, attachSearch, 25, attachOffset).then((items) => {
      if (active) {
        setAvailable(items);
        setTarget(items[0]?.id ?? "");
      }
    }).catch((e) => {
      if (active) setError(String(e));
    });
    return () => { active = false; };
  }, [selected?.id, kind, attachSearch, attachOffset]);

  const edit = (p: ResearchProject | "new") => {
    setEditing(p);
    setName(p === "new" ? "" : p.name);
    setDescription(p === "new" ? "" : p.description);
    setMaterial(p === "new" ? "" : p.materialSystem ?? "");
    setTags(p === "new" ? "" : p.tags.join(", "));
  };

  const save = async () => {
    try {
      const p = await desktop.saveResearchProject({
        ...(editing && editing !== "new" ? { id: editing.id, expectedUpdatedAt: editing.updatedAt } : {}),
        name,
        description,
        materialSystem: material || null,
        tags: tags.split(/[,，]/).map((s) => s.trim()).filter(Boolean),
      });
      setEditing(null);
      setSelected(p);
      await refresh();
      window.dispatchEvent(new Event("suna:projects-updated"));
    } catch (e) {
      setError(String(e));
    }
  };

  const action = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
      await loadDetails();
      await refresh();
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  const visible = objects.filter((o) =>
    tab === "总览" ||
    tab === "活动与溯源" ||
    (tab === "数据与知识" && ["dataset", "document", "knowledge_base"].includes(o.kind)) ||
    (tab === "模型与优化" && ["model", "optimization", "background_task"].includes(o.kind)) ||
    (tab === "实验" && o.kind === "experiment") ||
    (tab === "成果与报告" && ["report", "artifact"].includes(o.kind))
  );

  return (
    <section className="suna-projects-page">
      <header className="suna-projects-header">
        <div>
          <span className="suna-projects-kicker">RESEARCH WORKSPACE / 课题管理</span>
          <h1>科研项目</h1>
          <p>项目关联已有对象，归档与解绑保留原始数据。</p>
        </div>
        <Button variant="action" className="suna-new-project-btn" onClick={() => edit("new")}>
          <Plus size={16} aria-hidden="true" />
          新建科研项目
        </Button>
      </header>

      {error && <p className="suna-projects-alert" role="alert">{error}</p>}
      {loading && <p className="suna-projects-loading" role="status">正在读取科研项目…</p>}

      <ResearchWorkflowGuide project={selected} summary={summary} />

      <div className="suna-projects-filters">
        <div className="suna-projects-search-wrap">
          <Search size={15} className="suna-search-icon" aria-hidden="true" />
          <Input
            aria-label="搜索科研项目"
            value={search}
            onChange={(e) => { setSearch(e.target.value); setOffset(0); }}
            placeholder="搜索名称、目标或标签"
          />
        </div>
        <select
          aria-label="项目状态"
          className="suna-projects-status-select"
          value={status}
          onChange={(e) => { setStatus(e.target.value); setOffset(0); }}
        >
          <option value="active">活跃项目</option>
          <option value="archived">已归档</option>
          <option value="">全部状态</option>
        </select>
      </div>

      <div className="suna-projects-layout">
        <ResearchProjectListSidebar
          projects={projects}
          selected={selected}
          loading={loading}
          offset={offset}
          setOffset={setOffset}
          onOpen={open}
        />

        <ResearchProjectWorkspace
          selected={selected}
          summary={summary}
          objects={objects}
          visible={visible}
          labels={labels}
          tabs={tabs}
          tab={tab}
          setTab={setTab}
          kind={kind}
          setKind={setKind}
          target={target}
          setTarget={setTarget}
          available={available}
          attachSearch={attachSearch}
          setAttachSearch={setAttachSearch}
          attachOffset={attachOffset}
          setAttachOffset={setAttachOffset}
          objectOffset={objectOffset}
          setObjectOffset={setObjectOffset}
          edit={edit}
          action={action}
          setSelected={setSelected}
          setProject={setProject}
        />
      </div>

      <ResearchProjectEditorDialog
        editing={editing}
        name={name}
        description={description}
        material={material}
        tags={tags}
        setName={setName}
        setDescription={setDescription}
        setMaterial={setMaterial}
        setTags={setTags}
        onSave={() => void save()}
        onClose={() => setEditing(null)}
      />
    </section>
  );
}
