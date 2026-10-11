import { useEffect, useState } from "react";
import { Check, CircleAlert, Copy, FileCode2, Hash, LoaderCircle, Pencil, Plus, Puzzle, RefreshCw, Save, Tag, Trash2 } from "lucide-react";
import { desktop, type SkillCatalog, type SkillDraft, type SkillSummary } from "../../bridge/desktop";
import { useSkillStore } from "../../stores/skillStore";
import { useLocale } from "../../i18n/locale";
import { settingsErrorMessage } from "./settingsError";
import { Checkbox } from "../../components/ui/checkbox";
import { Input } from "../../components/ui/input";
import { Textarea } from "../../components/ui/textarea";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";

/**
 * 第 52/53 章：Skill 管理。
 *
 * 目录来自 skillStore；8 个内置 Skill 启动时种子到用户目录，因此全部
 * 支持页面内创建 / 编辑 / 复制 / 删除（53 章），启停上限 12 个（52 章）。
 */

type EditorState = { mode: "create" } | { mode: "edit"; original: string } | null;

const emptyDraft = (): SkillDraft => ({ name: "", description: "", version: "", tags: [], tools: [], knowledge_base: "", agent_id: "", model_id: "", body: "" });

export default function SettingsSkillsPanel() {
  const { t } = useLocale();
  // 第 77 章：Skill 目录由 skillStore 承载。
  const catalog = useSkillStore((state) => state.catalog);
  const setCatalog = useSkillStore((state) => state.setCatalog);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editor, setEditor] = useState<EditorState>(null);
  const [draft, setDraft] = useState<SkillDraft>(emptyDraft());

  const load = async () => { setLoading(true); setError(null); setNotice(null); try { setCatalog(await desktop.listSkills()); } catch (cause) { setError(settingsErrorMessage(cause, "无法读取 Skill 列表")); } finally { setLoading(false); } };
  useEffect(() => { void load(); }, []);

  const toggle = async (skill: SkillSummary) => { setBusy(skill.name); setError(null); setNotice(null); try { setCatalog(await desktop.setSkillEnabled(skill.name, !skill.enabled)); setNotice(`${skill.name} 已${skill.enabled ? "停用" : "启用"}`); } catch (cause) { setError(settingsErrorMessage(cause, "Skill 状态保存失败")); } finally { setBusy(null); } };

  const openCreate = () => { setDraft(emptyDraft()); setEditor({ mode: "create" }); setError(null); };
  const openEdit = (skill: SkillSummary) => {
    // 编辑器需要正文：从磁盘读取由 update 接口完成，这里先以目录元数据填充。
    setDraft({ name: skill.name, description: skill.description, version: skill.version, tags: skill.tags, tools: [], knowledge_base: "", agent_id: "", model_id: "", body: "" });
    setEditor({ mode: "edit", original: skill.name });
    setError(null);
  };

  const saveDraft = async () => {
    if (!editor) return;
    setBusy("editor"); setError(null); setNotice(null);
    const payload: SkillDraft = { ...draft, name: draft.name.trim(), description: draft.description.trim(), body: draft.body };
    try {
      const next = editor.mode === "create"
        ? await desktop.createSkill(payload)
        : await desktop.updateSkill(editor.original, payload);
      setCatalog(next);
      setNotice(editor.mode === "create" ? `Skill ${payload.name} 已创建` : `Skill ${editor.original} 已保存`);
      setEditor(null);
    } catch (cause) {
      setError(settingsErrorMessage(cause, "Skill 保存失败"));
    } finally { setBusy(null); }
  };

  const duplicate = async (skill: SkillSummary) => { setBusy(skill.name); setError(null); setNotice(null); try { setCatalog(await desktop.duplicateSkill(skill.name)); setNotice(`已复制为 ${skill.name}-copy`); } catch (cause) { setError(settingsErrorMessage(cause, "Skill 复制失败")); } finally { setBusy(null); } };
  const remove = async (skill: SkillSummary) => { setBusy(skill.name); setError(null); setNotice(null); try { setCatalog(await desktop.deleteSkill(skill.name)); setNotice(`Skill ${skill.name} 已删除`); } catch (cause) { setError(settingsErrorMessage(cause, "Skill 删除失败")); } finally { setBusy(null); } };

  const enabledCount = catalog.skills.filter((skill) => skill.enabled).length;
  if (loading) return <section className="suna-settings-form-panel" aria-busy="true"><p className="suna-settings-loading" role="status" aria-live="polite"><LoaderCircle size={16} className="suna-spin" />{t("loading")}</p></section>;
  return <section className="suna-settings-form-panel suna-settings-skill-panel" aria-labelledby="settings-skills-heading" aria-busy={busy !== null}><header className="suna-settings-form-heading"><div><span className="suna-settings-kicker">REUSABLE CAPABILITIES</span><h2 id="settings-skills-heading">{t("settingsCategorySkill")}</h2><p>管理可复用的专业研究能力、来源和加载状态。</p></div><div className="suna-settings-form-heading-actions"><Button size="icon" type="button" onClick={() => void load()} disabled={loading || busy !== null} aria-label="刷新 Skill"><RefreshCw size={17} /></Button><Button variant="ghost" type="button" onClick={openCreate} disabled={busy !== null}><Plus size={15} />新建 Skill</Button><Puzzle size={22} aria-hidden="true" /></div></header><p className="suna-skill-limit" role="status">已启用 {enabledCount}/12 个 Skill；Agent 每次运行最多加载 12 个 Skill。</p>{error && <p className="suna-settings-inline-error" role="alert">{error}</p>}{notice && <p className="suna-settings-notice" role="status"><Check size={14} />{notice}</p>}
    {editor && <form className="suna-skill-editor" onSubmit={(event) => { event.preventDefault(); void saveDraft(); }}>
      <div className="suna-skill-editor-head"><strong>{editor.mode === "create" ? "新建 Skill（第 53 章）" : `编辑 ${editor.original}`}</strong><Button size="icon" type="button" onClick={() => setEditor(null)} aria-label="取消编辑"><Trash2 size={14} /></Button></div>
      <label>名称（字母/数字/点/横线/下划线）<Input value={draft.name} disabled={editor.mode === "edit"} onChange={(event) => setDraft({ ...draft, name: event.target.value })} placeholder="wear-steel-analysis" required /></label>
      <label>描述<Input value={draft.description} onChange={(event) => setDraft({ ...draft, description: event.target.value })} placeholder="一句话说明该能力" required /></label>
      <label>版本<Input value={draft.version ?? ""} onChange={(event) => setDraft({ ...draft, version: event.target.value })} placeholder="1.0.0" /></label>
      <label>标签（逗号分隔，ASCII）<Input value={draft.tags.join(", ")} onChange={(event) => setDraft({ ...draft, tags: event.target.value.split(",").map((item) => item.trim()).filter(Boolean) })} placeholder="steel, analysis" /></label>
      <label>相关工具（逗号分隔，可选）<Input value={draft.tools.join(", ")} onChange={(event) => setDraft({ ...draft, tools: event.target.value.split(",").map((item) => item.trim()).filter(Boolean) })} placeholder="steel.knowledge_search" /></label>
      <label>知识库（可选）<Input value={draft.knowledge_base ?? ""} onChange={(event) => setDraft({ ...draft, knowledge_base: event.target.value })} placeholder="耐磨钢知识库" /></label>
      <label>Agent（可选）<Input value={draft.agent_id ?? ""} onChange={(event) => setDraft({ ...draft, agent_id: event.target.value })} placeholder="master" /></label>
      <label>模型（可选）<Input value={draft.model_id ?? ""} onChange={(event) => setDraft({ ...draft, model_id: event.target.value })} placeholder="gpt-4o" /></label>
      <label>工作流程提示（正文）<Textarea rows={8} value={draft.body} onChange={(event) => setDraft({ ...draft, body: event.target.value })} placeholder="输入/步骤/输出/边界…" required /></label>
      <Button variant="primary" type="submit" disabled={busy === "editor"}>{busy === "editor" ? <LoaderCircle size={15} className="suna-spin" /> : <Save size={15} />}保存</Button>
    </form>}
    {catalog.errors.length > 0 && <div className="suna-skill-errors" role="alert">{catalog.errors.map((item) => <p key={item.path}><CircleAlert size={14} />{item.path} · {item.message}</p>)}</div>}{catalog.skills.length === 0 ? <p className="suna-settings-empty"><Puzzle size={16} />{catalog.errors.length > 0 ? "未能加载可用 Skill，请修复上方错误后重试。" : t("extensionsNoSkills")}</p> : <div className="suna-settings-skill-list">{catalog.skills.map((skill) => <Card variant="settingsSkill" as="article" key={skill.name + "-" + skill.source.path}><div className="suna-settings-skill-card-head"><span className="suna-settings-skill-icon"><FileCode2 size={18} /></span><div><strong>{skill.name}</strong><p>{skill.description || "可复用的专业 Agent 能力"}</p></div><label className="suna-skill-switch"><Checkbox aria-label={`启用 ${skill.name}`} checked={skill.enabled} disabled={busy === skill.name} onCheckedChange={() => void toggle(skill)} /></label></div><div className="suna-settings-skill-meta"><span><Tag size={13} />{skill.tags.length ? skill.tags.join(" · ") : "未分类"}</span><span>v{skill.version}</span><span>{skill.enabled ? <><Check size={13} />已启用</> : "已停用"}</span></div><div className="suna-settings-skill-details"><span>来源：{skill.source.scope === "user" ? "用户 Skill" : skill.source.scope}</span><span>兼容：{skill.compatibility.length ? skill.compatibility.join(", ") : "通用"}</span><span title={skill.content_sha256}><Hash size={12} />{skill.content_sha256.slice(0, 16)}…</span></div><div className="suna-skill-card-actions">{skill.source.scope === "user" && <Button variant="ghost" type="button" onClick={() => openEdit(skill)} disabled={busy !== null}><Pencil size={13} />编辑</Button>}<Button variant="ghost" type="button" onClick={() => void duplicate(skill)} disabled={busy !== null}><Copy size={13} />复制</Button>{skill.source.scope === "user" && <Button variant="ghost" type="button" onClick={() => void remove(skill)} disabled={busy !== null}><Trash2 size={13} />删除</Button>}</div></Card>)}</div>}</section>;
}
