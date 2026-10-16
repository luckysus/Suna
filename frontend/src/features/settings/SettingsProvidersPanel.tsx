import { useState } from "react";
import { CircleHelp, Cloud, Filter, LoaderCircle, ShieldCheck, Sparkles, Zap } from "lucide-react";
import { useLocale } from "../../i18n/locale";
import SettingsProviderCard from "./SettingsProviderCard";
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
}) {
  const { t } = useLocale();
  const [filterSlot, setFilterSlot] = useState<string>("all");

  const visibleEditors = editors.filter((e) => filterSlot === "all" || e.slot === filterSlot);

  const slotTabNames: Record<string, string> = {
    chat: "对话 LLM",
    embedding: "向量嵌入",
    reranker: "语义重排",
    mineru: "文档解析",
  };

  return (
    <>
      <section className="suna-settings-plan" aria-labelledby="settings-plan-heading">
        <div className="suna-settings-plan-info">
          <div className="suna-settings-plan-title-row">
            <span className="suna-settings-plan-icon"><Cloud size={18} /></span>
            <div>
              <h2 id="settings-plan-heading">{t("settingsPlanTitle")}</h2>
              <p className="suna-settings-plan-sub">SiliconFlow 提供极速免运维的 BAAI 向量嵌入与语义重排端点支持。</p>
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
                <span className="suna-settings-plan-pill-spec">共享社区通道 · 免配密钥</span>
              </div>
            </label>
            <label className={`suna-settings-plan-pill ${plan === "pro" ? "is-selected" : ""}`}>
              <RadioGroupItem value="pro" aria-label={t("settingsPlanPro")} />
              <div className="suna-settings-plan-pill-body">
                <span className="suna-settings-plan-pill-title">{t("proPlan")}</span>
                <span className="suna-settings-plan-pill-spec">专享极速通道 · 高并发解析</span>
              </div>
            </label>
          </RadioGroup>
        </fieldset>
      </section>

      {editors.length > 1 && (
        <div className="suna-settings-slot-filter-bar">
          <span className="suna-settings-slot-filter-label"><Filter size={13} />筛选能力卡片：</span>
          <button
            type="button"
            className={`suna-settings-slot-filter-btn ${filterSlot === "all" ? "is-active" : ""}`}
            onClick={() => setFilterSlot("all")}
          >
            全部服务商 ({editors.length})
          </button>
          {editors.map((editor) => (
            <button
              key={editor.slot}
              type="button"
              className={`suna-settings-slot-filter-btn ${filterSlot === editor.slot ? "is-active" : ""}`}
              onClick={() => setFilterSlot(editor.slot)}
            >
              {slotTabNames[editor.slot] || editor.slot}
              <span className={`suna-slot-filter-dot ${editor.id && editor.enabled ? "is-ready" : "is-missing"}`} />
            </button>
          ))}
        </div>
      )}

      {loading ? <div className="suna-settings-loading"><LoaderCircle size={18} className="suna-spin" />{t("loading")}</div> : (
        <div className="suna-settings-grid">
          {visibleEditors.map((editor) => (
            <SettingsProviderCard
              key={editor.slot}
              editor={editor}
              busy={busySlot === editor.slot}
              testing={testingSlot === editor.slot}
              onChange={onChange}
              onSubmit={onSubmit}
              onTest={onTest}
              onDelete={onDelete}
            />
          ))}
        </div>
      )}

      <aside className="suna-settings-note"><CircleHelp size={17} aria-hidden="true" /><span>{t("settingsProviderNote")}</span></aside>
    </>
  );
}
