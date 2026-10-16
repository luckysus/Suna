import { FolderKanban } from "lucide-react";
import {
  desktop,
  type ResearchProject,
  type ResearchObject,
  type ResearchObjectKind,
  type ResearchSummary,
} from "../../bridge/desktop";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import ResearchMeasurementsPanel from "./ResearchMeasurementsPanel";
import ResearchResultsPanel from "./ResearchResultsPanel";
import WorkflowPanel from "./workflow/WorkflowPanel";
import { workflowService } from "./workflow/desktopService";

export interface ResearchProjectWorkspaceProps {
  selected: ResearchProject | null;
  summary: ResearchSummary | null;
  objects: ResearchObject[];
  visible: ResearchObject[];
  labels: Record<string, string>;
  tabs: string[];
  tab: string;
  setTab: (tab: string) => void;
  kind: ResearchObjectKind;
  setKind: (kind: ResearchObjectKind) => void;
  target: string;
  setTarget: (target: string) => void;
  available: ResearchObject[];
  attachSearch: string;
  setAttachSearch: (search: string) => void;
  attachOffset: number;
  setAttachOffset: (offsetOrFn: number | ((prev: number) => number)) => void;
  objectOffset: number;
  setObjectOffset: (offsetOrFn: number | ((prev: number) => number)) => void;
  edit: (p: ResearchProject) => void;
  action: (fn: () => Promise<unknown>) => Promise<void>;
  setSelected: (p: ResearchProject) => void;
  setProject: (p: ResearchProject) => void;
}

export default function ResearchProjectWorkspace({
  selected,
  summary,
  objects,
  visible,
  labels,
  tabs,
  tab,
  setTab,
  kind,
  setKind,
  target,
  setTarget,
  available,
  attachSearch,
  setAttachSearch,
  attachOffset,
  setAttachOffset,
  objectOffset,
  setObjectOffset,
  edit,
  action,
  setSelected,
  setProject,
}: ResearchProjectWorkspaceProps) {
  if (!selected) {
    return (
      <article className="suna-projects-detail-col suna-projects-empty-workspace">
        <div className="suna-empty-workspace-content">
          <div className="suna-empty-workspace-icon">
            <FolderKanban size={44} aria-hidden="true" />
          </div>
          <h2>选择或创建项目</h2>
          <p>从现有数据集、模型和任务开始组织课题。</p>
        </div>
      </article>
    );
  }

  return (
    <article className="suna-projects-detail-col">
      <header className="suna-project-detail-header">
        <div>
          <div className="suna-project-title-row">
            <h2>{selected.name}</h2>
            <span className={`suna-project-status-pill ${selected.status === "archived" ? "is-archived" : "is-active"}`}>
              {selected.status === "archived" ? "已归档" : "活跃"}
            </span>
            {selected.materialSystem && (
              <span className="suna-project-material-badge">{selected.materialSystem}</span>
            )}
          </div>
          <p className="suna-project-desc">{selected.description || "未填写科研目标与备注"}</p>
        </div>
        <div className="suna-project-actions">
          <Button
            variant="actionSecondary"
            disabled={selected.status === "archived"}
            onClick={() => edit(selected)}
          >
            编辑项目
          </Button>
          <Button
            variant="actionSecondary"
            onClick={() => void action(async () => {
              const p = await desktop.archiveResearchProject(selected.id, selected.status !== "archived");
              setSelected(p);
              window.dispatchEvent(new Event("suna:projects-updated"));
            })}
          >
            {selected.status === "archived" ? "恢复项目" : "归档项目"}
          </Button>
          <Button
            variant="action"
            disabled={selected.status === "archived"}
            onClick={() => void action(async () => {
              await desktop.setCurrentResearchProject(selected.id);
              setProject(selected);
              window.dispatchEvent(new Event("suna:projects-updated"));
            })}
          >
            设为当前项目
          </Button>
        </div>
      </header>

      <nav className="suna-project-detail-tabs" aria-label="科研项目详情">
        {tabs.map((t) => (
          <Button
            key={t}
            variant={t === tab ? "primary" : "ghost"}
            className={t === tab ? "is-active" : ""}
            onClick={() => setTab(t)}
          >
            {t}
          </Button>
        ))}
      </nav>

      {summary && (
        <div className="suna-project-summary-strip">
          <p>
            {Object.entries(summary.counts).map(([k, n]) => `${labels[k] ?? k} ${n}`).join(" · ") || "尚未关联对象"}
            {summary.missingSources > 0 ? ` · ${summary.missingSources} 个来源缺失` : ""}
          </p>
        </div>
      )}

      <fieldset className="suna-project-attach-bar" disabled={selected.status === "archived"}>
        <legend>关联已有对象</legend>
        <div className="suna-attach-controls">
          <select
            aria-label="关联对象类型"
            value={kind}
            onChange={(e) => { setKind(e.target.value as ResearchObjectKind); setAttachOffset(0); }}
          >
            {Object.entries(labels).filter(([k]) => k !== "artifact").map(([k, v]) => (
              <option key={k} value={k}>{v}</option>
            ))}
          </select>
          <Input
            aria-label="搜索可关联对象"
            placeholder="搜索已有对象"
            value={attachSearch}
            onChange={(e) => { setAttachSearch(e.target.value); setAttachOffset(0); }}
          />
          <select
            aria-label="关联已有对象"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
          >
            <option value="">选择当前工作区对象</option>
            {available.map((o) => (
              <option key={o.id} value={o.id}>{o.title}</option>
            ))}
          </select>
          <Button
            variant="action"
            disabled={!target}
            onClick={() => void action(() => desktop.attachResearchObject({ projectId: selected.id, kind, id: target }))}
          >
            关联到项目
          </Button>
        </div>
        <div className="suna-attach-pagination">
          <Button
            variant="ghost"
            disabled={!attachOffset}
            onClick={() => setAttachOffset(Math.max(0, attachOffset - 25))}
          >
            上一页可关联对象
          </Button>
          <Button
            variant="ghost"
            disabled={available.length < 25}
            onClick={() => setAttachOffset(attachOffset + 25)}
          >
            下一页可关联对象
          </Button>
        </div>
      </fieldset>

      {tab === "实验" && <ResearchMeasurementsPanel projectId={selected.id} />}
      {tab === "科研工作流" && (
        <WorkflowPanel
          key={selected.id}
          projectId={selected.id}
          archived={selected.status === "archived"}
          service={workflowService}
          onOpenPlan={() => setTab("实验")}
        />
      )}
      {(tab === "活动与溯源" || tab === "成果与报告") && <ResearchResultsPanel projectId={selected.id} />}

      <div className="suna-project-objects-section">
        <ul className="suna-project-objects-list">
          {visible.map((o) => (
            <li key={`${o.kind}:${o.id}:${o.relation}`} className="suna-project-object-item">
              <div className="suna-object-item-main">
                <span className="suna-object-kind-badge">{labels[o.kind]}</span>
                <strong>{o.title}</strong>
                <span className={`suna-object-relation ${o.availability === "missing" ? "is-missing" : ""}`}>
                  {o.availability === "missing" ? "来源已删除或不可用" : o.relation}
                </span>
              </div>
              <Button
                disabled={selected.status === "archived"}
                variant="ghost"
                className="suna-object-detach-btn"
                onClick={() => void action(() => desktop.detachResearchObject({ projectId: selected.id, kind: o.kind, id: o.id, relation: o.relation }))}
              >
                解除关联
              </Button>
            </li>
          ))}
        </ul>
        {!visible.length && <p className="suna-project-objects-empty">此视图没有已关联对象。</p>}
        <div className="suna-project-objects-pagination">
          <Button
            variant="ghost"
            disabled={!objectOffset}
            onClick={() => setObjectOffset(Math.max(0, objectOffset - 50))}
          >
            前一页对象
          </Button>
          <Button
            variant="ghost"
            disabled={objects.length < 50}
            onClick={() => setObjectOffset(objectOffset + 50)}
          >
            更多对象
          </Button>
        </div>
      </div>
    </article>
  );
}
