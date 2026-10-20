import { useEffect, useState } from "react";
import { FolderKanban } from "lucide-react";
import { desktop, type ResearchProject } from "../../bridge/desktop";
import { useResearchStore } from "../../stores/researchStore";
import { useLocale } from "../../i18n/locale";

export default function ResearchProjectContext() {
    const { t } = useLocale();
    const { currentProject, setProject } = useResearchStore();
    const [projects, setProjects] = useState<ResearchProject[]>([]);
    const [error, setError] = useState("");
    const load = async () => {
        try {
            const [list, current] = await Promise.all([
                desktop.listResearchProjects({ status: "active", limit: 100 }),
                desktop.getCurrentResearchProject()
            ]);
            setProjects(list);
            setProject(current);
            setError("");
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        }
    };
    useEffect(() => {
        void load();
        const refresh = () => { void load(); };
        window.addEventListener("suna:projects-updated", refresh);
        return () => window.removeEventListener("suna:projects-updated", refresh);
    }, []);
    const select = async (id: string) => {
        try {
            await desktop.setCurrentResearchProject(id || null);
            setProject(projects.find((p) => p.id === id) ?? null);
            setError("");
        } catch (e) {
            setError(String(e));
        }
    };
    return (
        <div className="suna-project-context">
            <div className="suna-project-context-inner">
                <span className={`suna-project-context-indicator ${currentProject ? "is-active" : ""}`} />
                <label>
                    <FolderKanban size={14} aria-hidden="true" />
                    <span>{t("projectContextLabel")}</span>
                    <select
                        aria-label={t("projectContextLabel")}
                        value={currentProject?.id ?? ""}
                        onChange={(e) => void select(e.target.value)}
                    >
                        <option value="">{t("projectContextUnspecified")}</option>
                        {projects.map((p) => (
                            <option key={p.id} value={p.id}>{p.name}</option>
                        ))}
                        {currentProject && !projects.some(p => p.id === currentProject.id) && (
                            <option value={currentProject.id}>
                                {currentProject.name}{currentProject.status === "archived" ? ` (${t("projectContextArchived")})` : ""}
                            </option>
                        )}
                    </select>
                </label>
                {currentProject && (
                    <span className="suna-project-context-tag">
                        {currentProject.materialSystem || t("projectContextActive")}
                    </span>
                )}
            </div>
            {error && <span role="status" className="suna-project-context-error">{t("projectContextUnavailable")}</span>}
        </div>
    );
}
