import type { ModelUsageDetails } from "../../../bridge/desktop";
import { usageBindingLabel } from "./modelUsage";
import { Button } from "../../../components/ui/button";

export default function ModelInUseDialog({
  details,
  onClose,
}: {
  details: ModelUsageDetails;
  onClose: () => void;
}) {
  return (
    <div className="suna-model-inuse-overlay" role="presentation" onClick={onClose}>
      <div
        className="suna-model-inuse-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="model-inuse-title"
        onClick={(event) => event.stopPropagation()}
      >
        <h3 id="model-inuse-title">无法删除：模型仍在使用中</h3>
        <p>请先在下列资源中更换模型或解除绑定，然后再删除。</p>

        {details.long_term_memory.bindings.length > 0 && (
          <section>
            <h4>系统默认 / 偏好设置</h4>
            <ul>
              {details.long_term_memory.bindings.map((binding) => (
                <li key={binding}>{usageBindingLabel(binding)}</li>
              ))}
            </ul>
          </section>
        )}

        {details.knowledge_bases.length > 0 && (
          <section>
            <h4>
              知识库（{details.knowledge_bases.length}
              {details.knowledge_base_total > details.knowledge_bases.length
                ? ` / 共 ${details.knowledge_base_total}`
                : ""}
              ）
            </h4>
            <ul>
              {details.knowledge_bases.map((kb) => (
                <li key={kb.id}>
                  <strong>{kb.name}</strong>
                  <span>{kb.bindings.map(usageBindingLabel).join("、")}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {details.agents.length > 0 && (
          <section>
            <h4>
              智能体（{details.agents.length}
              {details.agent_total > details.agents.length ? ` / 共 ${details.agent_total}` : ""}
              ）
            </h4>
            <ul>
              {details.agents.map((agent) => (
                <li key={agent.id}>
                  <strong>{agent.name}</strong>
                  <span>{agent.bindings.map(usageBindingLabel).join("、")}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        <footer className="suna-model-inuse-actions">
          {details.knowledge_bases.length > 0 && (
            <Button
              type="button"
              variant="secondary"
              onClick={() => {
                window.dispatchEvent(new CustomEvent("suna:navigate-section", { detail: { section: "knowledge" } }));
                onClose();
              }}
            >
              打开知识中心
            </Button>
          )}
          {details.agents.length > 0 && (
            <Button
              type="button"
              variant="secondary"
              onClick={() => {
                window.dispatchEvent(new CustomEvent("suna:navigate-section", { detail: { section: "agents" } }));
                onClose();
              }}
            >
              打开 Agent 管理
            </Button>
          )}
          <Button type="button" variant="primary" onClick={onClose}>知道了</Button>
        </footer>
      </div>
    </div>
  );
}
