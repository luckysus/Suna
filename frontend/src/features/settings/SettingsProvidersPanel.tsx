import { useState } from "react";
import { Bot, Check, CircleHelp, Cloud, Database, LoaderCircle, SlidersHorizontal, Sparkles } from "lucide-react";
import { useLocale } from "../../i18n/locale";
import SettingsProviderCard from "./SettingsProviderCard";
import ModelRuntimeSettings from "./ModelRuntimeSettings";
import { RadioGroup, RadioGroupItem } from "../../components/ui/radio-group";
import type { ProviderSlot, RetrievalPlan, SettingsEditor } from "./settingsModel";

export default function SettingsProvidersPanel({
  plan,
  loading,
  editors,
  busySlot,
  testingSlot,
  onChange,
  onSubmit,
  onTest,
  onDelete,
  onPlanChange,
  showRuntimeTab,
}: {
  plan: RetrievalPlan;
  loading: boolean;
  editors: SettingsEditor[];
  busySlot: ProviderSlot | null;
  testingSlot: ProviderSlot | null;
  onChange: (editor: SettingsEditor) => void;
  onSubmit: (event: React.FormEvent<HTMLFormElement>, editor: SettingsEditor) => void;
  onTest: (editor: SettingsEditor) => void;
  onDelete: (editor: SettingsEditor) => void;
  onPlanChange: (plan: RetrievalPlan) => void;
  showRuntimeTab?: boolean;
}) {
  const { t } = useLocale();
  const defaultSlot = editors[0]?.slot ?? "chat";
  const [activeTab, setActiveTab] = useState<string>(defaultSlot);

  const slotTabNames: Record<string, string> = {
    chat: "对话推理",
    embedding: "向量嵌入",
    reranker: "语义重排",
    mineru: "文档解析",
  };

  return (
    <>
      <div className="suna-model-segmented-bar" role="tablist" aria-label="模型能力与运行配置">
        {editors.map((editor) => {
          const isReady = Boolean(editor.id && editor.enabled);
          const active = activeTab === editor.slot;
          return (
            <button
              key={editor.slot}
              type="button"
              role="tab"
              aria-selected={active}
              className={`suna-model-segment-btn ${active ? "is-active" : ""}`}
              onClick={() => setActiveTab(editor.slot)}
            >
              <span className={`suna-model-segment-icon slot-${editor.slot}`}>
                {editor.slot === "chat" ? <Bot size={15} /> : editor.slot === "embedding" ? <Sparkles size={15} /> : editor.slot === "reranker" ? <Check size={15} /> : <Database size={15} />}
              </span>
              <div className="suna-model-segment-text">
                <div className="suna-model-segment-title-row">
                  <span className="suna-model-segment-name">{slotTabNames[editor.slot] || editor.slot}</span>
                  <span className={`suna-model-segment-dot ${isReady ? "is-ready" : "is-missing"}`} />
                </div>
                <span className="suna-model-segment-sub">
                  {editor.modelId ? editor.modelId.split("/").pop() : isReady ? "已就绪" : "待配置"}
                </span>
              </div>
            </button>
          );
        })}

        <div className="suna-model-segment-divider" />

        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "plan"}
          className={`suna-model-segment-btn ${activeTab === "plan" ? "is-active" : ""}`}
          onClick={() => setActiveTab("plan")}
        >
          <span className="suna-model-segment-icon slot-plan"><Cloud size={15} /></span>
          <div className="suna-model-segment-text">
            <div className="suna-model-segment-title-row">
              <span className="suna-model-segment-name">云端专线</span>
              <span className="suna-model-segment-dot is-ready" />
            </div>
            <span className="suna-model-segment-sub">{plan === "pro" ? "Pro 专线通道" : "Free 免费通道"}</span>
          </div>
        </button>

        {showRuntimeTab && (
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === "runtime"}
            className={`suna-model-segment-btn ${activeTab === "runtime" ? "is-active" : ""}`}
            onClick={() => setActiveTab("runtime")}
          >
            <span className="suna-model-segment-icon slot-runtime"><SlidersHorizontal size={15} /></span>
            <div className="suna-model-segment-text">
              <div className="suna-model-segment-title-row">
                <span className="suna-model-segment-name">推理参数</span>
                <span className="suna-model-segment-dot is-ready" />
              </div>
              <span className="suna-model-segment-sub">采样温 / 上下文</span>
            </div>
          </button>
        )}
      </div>

      {loading ? (
        <div className="suna-settings-loading">
          <LoaderCircle size={18} className="suna-spin" />
          {t("loading")}
        </div>
      ) : (
        <div className="suna-model-workspace-stage">
          {editors.map((editor) => (
            <div
              key={editor.slot}
              className={`suna-model-tab-pane ${activeTab === editor.slot ? "is-active" : "is-inactive"}`}
            >
              <SettingsProviderCard
                editor={editor}
                busy={busySlot === editor.slot}
                testing={testingSlot === editor.slot}
                onChange={onChange}
                onSubmit={onSubmit}
                onTest={onTest}
                onDelete={onDelete}
              />
            </div>
          ))}

          <div className={`suna-model-tab-pane ${activeTab === "plan" ? "is-active" : "is-inactive"}`}>
            <section className="suna-settings-plan" aria-labelledby="settings-plan-heading">
              <div className="suna-settings-plan-info">
                <div className="suna-settings-plan-title-row">
                  <span className="suna-settings-plan-icon"><Cloud size={18} /></span>
                  <div>
                    <span className="suna-settings-card-kicker">CLOUD RETRIEVAL TIER</span>
                    <h2 id="settings-plan-heading">{t("settingsPlanTitle")}</h2>
                    <p className="suna-settings-plan-sub">SiliconFlow 提供极速免运维的 BAAI 向量嵌入与语义重排端点支持，无需本地配置显卡算力。</p>
                  </div>
                </div>
              </div>
              <fieldset className="suna-settings-plan-options">
                <legend>{t("settingsPlanLabel")}</legend>
                <RadioGroup value={plan} onValueChange={(value) => onPlanChange(value as RetrievalPlan)} aria-label={t("settingsPlanLabel")}>
                  <label className={`suna-settings-plan-pill ${plan === "free" ? "is-selected" : ""}`}>
                    <RadioGroupItem value="free" aria-label={t("settingsPlanFree")} />
                    <div className="suna-settings-plan-pill-body">
                      <span className="suna-settings-plan-pill-title">{t("freePlan")}</span>
                      <span className="suna-settings-plan-pill-spec">共享社区通道 · 基础响应延迟 · 适合日常研发测试</span>
                    </div>
                  </label>
                  <label className={`suna-settings-plan-pill ${plan === "pro" ? "is-selected" : ""}`}>
                    <RadioGroupItem value="pro" aria-label={t("settingsPlanPro")} />
                    <div className="suna-settings-plan-pill-body">
                      <span className="suna-settings-plan-pill-title">{t("proPlan")}</span>
                      <span className="suna-settings-plan-pill-spec">专享极速通道 · 高并发保障 · 适合海量文献抽取</span>
                    </div>
                  </label>
                </RadioGroup>
              </fieldset>
            </section>
          </div>

          {showRuntimeTab && (
            <div className={`suna-model-tab-pane ${activeTab === "runtime" ? "is-active" : "is-inactive"}`}>
              <ModelRuntimeSettings />
            </div>
          )}
        </div>
      )}

      <aside className="suna-settings-note">
        <CircleHelp size={17} aria-hidden="true" />
        <span>{t("settingsProviderNote")}</span>
      </aside>
    </>
  );
}
