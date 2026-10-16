import { useEffect, useState } from "react";
import { FolderKanban } from "lucide-react";
import { desktop, type ResearchProject } from "../../bridge/desktop";
import { useResearchStore } from "../../stores/researchStore";

export default function ResearchProjectContext() {
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
                    <span>当前科研项目</span>
                    <select
                        aria-label="当前科研项目"
                        value={currentProject?.id ?? ""}
                        onChange={(e) => void select(e.target.value)}
                    >
                        <option value="">未指定项目</option>
                        {projects.map((p) => (
                            <option key={p.id} value={p.id}>{p.name}</option>
                        ))}
                        {currentProject && !projects.some(p => p.id === currentProject.id) && (
                            <option value={currentProject.id}>
                                {currentProject.name}{currentProject.status === "archived" ? "（已归档）" : ""}
                            </option>
                        )}
                    </select>
                </label>
                {currentProject && (
                    <span className="suna-project-context-tag">
                        {currentProject.materialSystem || "课题上下文生效中"}
                    </span>
                )}
            </div>
            {error && <span role="status" className="suna-project-context-error">项目上下文暂不可用</span>}
        </div>
    );
}
