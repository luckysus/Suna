import { useCallback, useEffect, useState } from "react";
import type { CatalogProvider, ProfileModelType } from "../../../bridge/desktop";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";

const cache = new Map<string, CatalogProvider[]>();
const inflight = new Map<string, Promise<CatalogProvider[]>>();

export function providerLabel(provider: CatalogProvider | undefined, locale = "zh-CN"): string {
  if (!provider) return "";
  return provider.labels?.[locale] ?? provider.label;
}

export async function ensureModelProvidersLoaded(modelType?: ProfileModelType | "all"): Promise<CatalogProvider[]> {
  const key = modelType && modelType !== "all" ? modelType : "";
  if (cache.has(key)) return cache.get(key)!;
  const pending = inflight.get(key);
  if (pending) return pending;
  const request = (async () => {
    if (!isDesktopRuntime()) {
      return [];
    }
    const listCatalog = desktop.listModelProviderCatalog;
    if (!listCatalog) {
      return [];
    }
    const providers = await listCatalog(modelType === "all" ? null : modelType ?? null);
    cache.set(key, providers);
    return providers;
  })();
  inflight.set(key, request);
  try {
    return await request;
  } finally {
    inflight.delete(key);
  }
}

export function providerByIdFromCache(id?: string | null) {
  if (!id) return undefined;
  for (const list of cache.values()) {
    const match = list.find((entry) => entry.id === id);
    if (match) return match;
  }
  return undefined;
}

export function useModelProviders(modelType?: ProfileModelType | "all") {
  const [providers, setProviders] = useState<CatalogProvider[]>([]);
  const [loading, setLoading] = useState(true);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      cache.delete(modelType && modelType !== "all" ? modelType : "");
      const list = await ensureModelProvidersLoaded(modelType);
      setProviders(list);
    } finally {
      setLoading(false);
    }
  }, [modelType]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const providerById = useCallback(
    (id?: string | null) => providers.find((entry) => entry.id === id),
    [providers],
  );

  return { providers, loading, reload, providerById, providerLabel };
}
