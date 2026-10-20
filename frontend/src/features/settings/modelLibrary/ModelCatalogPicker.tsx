import { useEffect, useMemo, useState } from "react";
import type { CatalogModelEntry, CatalogProvider, ProfileModelType, ProviderKind } from "../../../bridge/desktop";
import { providerLabel, useModelProviders } from "./useModelProviders";
import { Button } from "../../../components/ui/button";

export default function ModelCatalogPicker({
  modelType,
  kind,
  modelId,
  onApply,
}: {
  modelType: ProfileModelType;
  kind: ProviderKind;
  modelId: string;
  onApply: (patch: {
    providerId: string;
    kind: ProviderKind;
    displayName: string;
    baseUrl: string;
    modelId: string;
  }) => void;
}) {
  const { providers, loading } = useModelProviders(modelType);
  const [activeId, setActiveId] = useState<string | null>(null);
  const visibleProviders = useMemo(
    () => (modelType === "rerank" ? providers.filter((entry) => entry.kind !== "ollama") : providers),
    [modelType, providers],
  );

  useEffect(() => {
    const match = visibleProviders.find((entry) => entry.kind === kind);
    const nextId = match?.id ?? visibleProviders[0]?.id ?? null;
    setActiveId((current) => (current === nextId ? current : nextId));
  }, [visibleProviders, kind]);
  const activeProvider = visibleProviders.find((entry) => entry.id === activeId) ?? visibleProviders[0];

  const applyProvider = (provider: CatalogProvider) => {
    const baseUrl = provider.default_urls?.[modelType] ?? provider.default_urls?.chat ?? "";
    onApply({
      providerId: provider.id,
      kind: provider.kind,
      displayName: providerLabel(provider),
      baseUrl,
      modelId: modelId || provider.models?.[0]?.id || "",
    });
  };

  const applyModel = (provider: CatalogProvider, model: CatalogModelEntry) => {
    onApply({
      providerId: provider.id,
      kind: provider.kind,
      displayName: `${providerLabel(provider)} · ${model.name}`,
      baseUrl: provider.default_urls?.[modelType] ?? provider.default_urls?.chat ?? "",
      modelId: model.id,
    });
  };

  if (loading) {
    return <p className="suna-settings-field-hint">正在加载服务商目录…</p>;
  }
  if (visibleProviders.length === 0) {
    return null;
  }

  return (
    <section className="suna-model-catalog-picker" aria-label="内置模型目录">
      <div className="suna-model-catalog-providers">
        {visibleProviders.map((provider) => (
          <button
            key={provider.id}
            type="button"
            className={`suna-model-catalog-provider ${activeProvider?.id === provider.id ? "is-active" : ""}`}
            onClick={() => {
              setActiveId(provider.id);
              applyProvider(provider);
            }}
          >
            {provider.icon_svg && (
              <span className="suna-model-catalog-icon" aria-hidden dangerouslySetInnerHTML={{ __html: provider.icon_svg }} />
            )}
            {providerLabel(provider)}
          </button>
        ))}
      </div>
      {activeProvider && (activeProvider.models?.length ?? 0) > 0 && (
        <div className="suna-model-catalog-models">
          {activeProvider.models!.map((model) => (
            <Button
              key={model.id}
              type="button"
              variant={modelId === model.id ? "primary" : "secondary"}
              onClick={() => applyModel(activeProvider, model)}
            >
              {model.name}
            </Button>
          ))}
        </div>
      )}
    </section>
  );
}
