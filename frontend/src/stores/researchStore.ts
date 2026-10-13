import { create } from "zustand";
import type { ResearchProject } from "../bridge/desktop";
export const useResearchStore = create<{ currentProject: ResearchProject | null; setProject: (project: ResearchProject | null) => void }>((set) => ({ currentProject: null, setProject: (currentProject) => set({ currentProject }) }));
