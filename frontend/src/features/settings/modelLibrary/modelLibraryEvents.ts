export const openModelLibraryEvent = "suna:open-model-library";
export const modelLibraryFocusModelEvent = "suna:model-library-focus";

export type OpenModelLibraryDetail = {
  modelId?: string;
  modelType?: string;
};

export function openModelLibrary(detail: OpenModelLibraryDetail = {}) {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(openModelLibraryEvent, { detail }));
}

export function focusModelInLibrary(modelId: string) {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(modelLibraryFocusModelEvent, { detail: { modelId } }));
}
