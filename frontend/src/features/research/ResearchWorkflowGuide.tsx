import type { ResearchProject, ResearchSummary } from "../../bridge/desktop";
import { useResearchStore } from "../../stores/researchStore";
import { Compass, AlertCircle, ArrowUpRight } from "lucide-react";

export default function ResearchWorkflowGuide({
  project,
  summary,
}: {
  project: ResearchProject | null;
  summary: ResearchSummary | null;
}) {
  const current = useResearchStore((s) => s.currentProject);
  const count = (kind: string) =>
    summary ? `已关联 ${summary.counts[kind] ?? 0} 项` : "尚未读取项目记录";

  return (
    <details className="suna-research-guide" open={!project}>
      <summary className="suna-research-guide-summary">
        <div className="suna-research-guide-header">
          <Compass size={17} className="suna-guide-icon" aria-hidden="true" />
          <span className="suna-guide-title">科研入门与离线流程</span>
          <span className="suna-guide-badge">6 阶段研发闭环管线</span>
        </div>
      </summary>

      <div className="suna-research-guide-body">
        <p className="suna-research-guide-lead">
          {project
            ? `已打开 ${project.name}。${
                current?.id === project.id
                  ? "该项目是当前科研上下文。"
                  : "请先使用“设为当前项目”，再提交计算任务。"
              }`
            : "先新建项目并设为当前项目，再按下面的步骤完成第一轮研究。"}
        </p>

        <ol className="suna-research-guide-steps">
          <li>
            <div className="suna-guide-step-card">
              <span className="suna-step-badge">01</span>
              <div className="suna-step-detail">
                <a href="/diagnostics">
                  检查本机计算 Worker <ArrowUpRight size={13} aria-hidden="true" />
                </a>
                <span>实际加载依赖；基础数据处理和本机计算可离线使用。</span>
              </div>
            </div>
          </li>
          <li>
            <div className="suna-guide-step-card">
              <span className="suna-step-badge">02</span>
              <div className="suna-step-detail">
                <a href="/data">
                  导入数据与字段映射 <ArrowUpRight size={13} aria-hidden="true" />
                </a>
                <span>
                  <em className="suna-count-tag">{count("dataset")}</em>。确认单位、特征/目标及炉次字段，再激活草稿。
                </span>
              </div>
            </div>
          </li>
          <li>
            <div className="suna-guide-step-card">
              <span className="suna-step-badge">03</span>
              <div className="suna-step-detail">
                <a href="/prediction">
                  训练性能模型 <ArrowUpRight size={13} aria-hidden="true" />
                </a>
                <span>
                  <em className="suna-count-tag">{count("model")}</em>。优先按炉次分组评估，训练结果需核对外推风险。
                </span>
              </div>
            </div>
          </li>
          <li>
            <div className="suna-guide-step-card">
              <span className="suna-step-badge">04</span>
              <div className="suna-step-detail">
                <a href="/optimization">
                  联合优化与专业约束 <ArrowUpRight size={13} aria-hidden="true" />
                </a>
                <span>
                  <em className="suna-count-tag">{count("optimization")}</em>。逐个模型设置性能目标和方向，核对约束来源。
                </span>
              </div>
            </div>
          </li>
          <li>
            <div className="suna-guide-step-card">
              <span className="suna-step-badge">05</span>
              <div className="suna-step-detail">
                <a href="/experiment">
                  实验计划与测量回录 <ArrowUpRight size={13} aria-hidden="true" />
                </a>
                <span>
                  <em className="suna-count-tag">{count("experiment")}</em>。在项目“实验”中回录、审核，生成新版本后显式再训练。
                </span>
              </div>
            </div>
          </li>
          <li>
            <div className="suna-guide-step-card">
              <span className="suna-step-badge">06</span>
              <div className="suna-step-detail">
                <a href="/reports">
                  成果与报告 <ArrowUpRight size={13} aria-hidden="true" />
                </a>
                <span>
                  <em className="suna-count-tag">{count("report")}</em>。在项目“成果与报告”生成报告/manifest，原始数据备份走运行记录。
                </span>
              </div>
            </div>
          </li>
        </ol>

        {summary && summary.missingSources > 0 && (
          <p role="status" className="suna-guide-status-alert">
            <AlertCircle size={15} aria-hidden="true" />
            <span>本项目有 {summary.missingSources} 项来源缺失，请先在活动与溯源中核对。</span>
          </p>
        )}

        <p className="suna-guide-ext-footer">
          <a href="/data?tab=microstructure">显微组织</a>、
          <a href="/data?tab=sources">专业数据源</a>和
          <a href="/literature">文献证据</a>
          可按研究需要接入。AI 对话、远端向量和公开题录检索需要各自配置及联网授权；关闭联网后仍可读取本地记录。
        </p>
      </div>
    </details>
  );
}
