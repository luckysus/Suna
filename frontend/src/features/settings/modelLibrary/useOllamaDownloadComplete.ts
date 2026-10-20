import { useEffect } from "react";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";
import { emitProviderProfilesChanged } from "../settingsModel";
import { inferProfileModelType } from "./modelLibraryUtils";

export function useOllamaDownloadComplete() {
  useEffect(() => {
    if (!isDesktopRuntime()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const onProgress = (event: { status?: string; model_name?: string } | null | undefined) => {
      if (!event?.status?.toLowerCase().includes("success")) return;
      const modelName = event.model_name?.trim();
      if (!modelName) return;
      void desktop.listProviderProfiles().then((profiles) => {
        if (disposed || !Array.isArray(profiles)) return;
        const targets = profiles.filter(
          (profile) =>
            profile.kind === "ollama"
            && profile.status === "downloading"
            && profile.model_id === modelName,
        );
        if (targets.length === 0) return;
        void Promise.all(
          targets.map((profile) =>
            desktop.saveProviderProfile({
              id: profile.id,
              kind: profile.kind,
              display_name: profile.display_name,
              base_url: profile.base_url,
              model_id: profile.model_id,
              enabled: profile.enabled,
              model_type: inferProfileModelType(profile),
              source: profile.source ?? "local",
              status: "active",
              metadata_json: profile.metadata_json,
            }),
          ),
        ).then(() => {
          if (!disposed) emitProviderProfilesChanged();
        });
      });
    };

    void desktop
      .listenOllamaDownloadProgress(onProgress)
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
