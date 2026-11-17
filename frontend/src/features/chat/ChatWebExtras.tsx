import { useState } from "react";
import { ChevronDown, ChevronRight, Globe, ListTree, Search } from "lucide-react";
import type { WebQueryRewrite, WebResearchSnapshot, WebSource } from "./web/webTypes";

export function WebSourcesBar({ sources }: { sources: WebSource[] }) {
  const [open, setOpen] = useState(false);
  if (!sources.length) return null;
  return (
    <div className="suna-chat-web-sources">
      <button type="button" className="suna-chat-chip-toggle" onClick={() => setOpen((value) => !value)}>
        <Globe size={14} aria-hidden="true" />
        <span>已搜索 {sources.length} 个网页</span>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      </button>
      {open ? (
        <div className="suna-chat-web-sources-list">
          {sources.map((source) => (
            <a
              key={`${source.index}-${source.url}`}
              href={source.url}
              target="_blank"
              rel="noreferrer"
              title={source.title}
              className="suna-chat-web-source-link"
            >
              <span>{source.index}</span>
              <em>{source.title}</em>
            </a>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export function QueryRewriteBar({ rewrite }: { rewrite?: WebQueryRewrite | null }) {
  const [open, setOpen] = useState(false);
  const hasEnhancements = Boolean(
    rewrite?.alias_queries?.length
      || rewrite?.multi_queries?.length
      || rewrite?.hyde_used
      || rewrite?.step_back_query
      || rewrite?.decomposed_queries?.length,
  );
  if (!rewrite || (!rewrite.changed && !hasEnhancements)) return null;
  return (
    <div className="suna-chat-query-rewrite">
      <button type="button" className="suna-chat-chip-toggle" onClick={() => setOpen((value) => !value)}>
        <Search size={13} aria-hidden="true" />
        <span>检索问题已优化</span>
        {open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
      </button>
      {open ? (
        <div className="suna-chat-query-rewrite-body">
          <div><strong>用户原问：</strong>{rewrite.original_query}</div>
          <div><strong>实际检索：</strong>{rewrite.retrieval_query}</div>
          {rewrite.alias_queries?.length ? <div><strong>同义词扩展：</strong>{rewrite.alias_queries.length} 个</div> : null}
          {rewrite.multi_queries?.length ? <div><strong>多路检索：</strong>{rewrite.multi_queries.length} 路</div> : null}
          {rewrite.step_back_query ? <div><strong>回溯问题：</strong>{rewrite.step_back_query}</div> : null}
          {rewrite.decomposed_queries?.length ? <div><strong>分解问题：</strong>{rewrite.decomposed_queries.join("；")}</div> : null}
          {rewrite.hyde_used ? <div>已启用 HyDE</div> : null}
        </div>
      ) : null}
    </div>
  );
}

export function ResearchTracePanel({ snapshot }: { snapshot?: WebResearchSnapshot | null }) {
  const [open, setOpen] = useState(false);
  if (!snapshot) return null;
  const tasks = snapshot.tasks ?? [];
  const evidence = snapshot.evidence ?? [];
  const decisions = snapshot.decisions ?? [];
  if (!snapshot.goal && !tasks.length && !evidence.length && !decisions.length) return null;
  return (
    <div className="suna-chat-research-trace">
      <button type="button" className="suna-chat-chip-toggle" onClick={() => setOpen((value) => !value)}>
        <ListTree size={14} aria-hidden="true" />
        <span>研究轨迹{snapshot.status ? ` · ${snapshot.status}` : ""}</span>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      </button>
      {open ? (
        <div className="suna-chat-research-trace-body">
          {snapshot.goal ? <div><strong>目标：</strong>{snapshot.goal}</div> : null}
          {snapshot.plan ? <div><strong>计划：</strong>{snapshot.plan}</div> : null}
          {tasks.length ? (
            <ul>
              {tasks.slice(0, 8).map((task, index) => (
                <li key={`${task}-${index}`}>{task}</li>
              ))}
            </ul>
          ) : null}
          {evidence.length ? <div><strong>证据：</strong>{evidence.length} 条</div> : null}
          {decisions.length ? <div><strong>决策：</strong>{decisions.join("；")}</div> : null}
          {snapshot.verification_issues?.length ? (
            <div><strong>校验问题：</strong>{snapshot.verification_issues.join("；")}</div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
