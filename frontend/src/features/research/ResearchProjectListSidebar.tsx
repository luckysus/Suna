import { FolderPlus } from "lucide-react";
import type { ResearchProject } from "../../bridge/desktop";
import { Button } from "../../components/ui/button";

export interface ResearchProjectListSidebarProps {
  projects: ResearchProject[];
  selected: ResearchProject | null;
  loading: boolean;
  offset: number;
  setOffset: (offsetOrFn: number | ((prev: number) => number)) => void;
  onOpen: (id: string) => void;
}

export default function ResearchProjectListSidebar({
  projects,
  selected,
  loading,
  offset,
  setOffset,
  onOpen,
}: ResearchProjectListSidebarProps) {
  return (
    <aside className="suna-projects-list-col">
      <div className="suna-projects-list-header">
        <span>课题列表 ({projects.length})</span>
      </div>
      <div className="suna-projects-card-list">
        {projects.map((p) => (
          <button
            key={p.id}
            className={`suna-project-card-btn ${selected?.id === p.id ? "is-active" : ""}`}
            onClick={() => void onOpen(p.id)}
          >
            <div className="suna-project-card-top">
              <strong>{p.name}</strong>
              <span className={`suna-project-status-pill ${p.status === "archived" ? "is-archived" : "is-active"}`}>
                {p.status === "archived" ? "已归档" : "活跃"}
              </span>
            </div>
            <span className="suna-project-card-material">
              {p.materialSystem || "材料体系未指定"}
            </span>
            {p.tags.length > 0 && (
              <div className="suna-project-card-tags">
                <small>{p.tags.join(" · ")}</small>
              </div>
            )}
          </button>
        ))}
        {!loading && !projects.length && (
          <div className="suna-projects-empty-left">
            <FolderPlus size={32} aria-hidden="true" />
            <p>暂无科研项目，创建项目后关联已有数据和任务。</p>
          </div>
        )}
      </div>
      <div className="suna-projects-pagination">
        <Button
          variant="actionSecondary"
          disabled={!offset}
          onClick={() => setOffset(Math.max(0, offset - 25))}
        >
          上一页
        </Button>
        <span className="suna-projects-page-indicator">第 {Math.floor(offset / 25) + 1} 页</span>
        <Button
          variant="actionSecondary"
          disabled={projects.length < 25}
          onClick={() => setOffset(offset + 25)}
        >
          下一页
        </Button>
      </div>
    </aside>
  );
}
