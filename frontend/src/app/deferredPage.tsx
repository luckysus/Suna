import { useEffect, useState, type ComponentType } from 'react';

export type DeferredComponent<Props extends object> = ComponentType<Props> & {
  preload: () => Promise<ComponentType<Props>>;
};

/** Load local page modules on demand and cache only successful imports. */
export function deferredPage<Props extends object>(
  importer: () => Promise<{ default: ComponentType<Props> }>,
  reload: () => void = () => window.location.reload(),
): DeferredComponent<Props> {
  let resolved: ComponentType<Props> | null = null;
  let pending: Promise<ComponentType<Props>> | null = null;
  const load = () => {
    if (resolved) return Promise.resolve(resolved);
    if (!pending) {
      const request = Promise.resolve()
        .then(importer)
        .then((module) => {
          if (!module.default) throw new Error('page_module_unavailable');
          resolved = module.default;
          return resolved;
        })
        .finally(() => {
          if (pending === request) pending = null;
        });
      pending = request;
    }
    return pending;
  };
  const DeferredPage: DeferredComponent<Props> = function DeferredPage(props: Props) {
    const [Page, setPage] = useState<ComponentType<Props> | null>(() => resolved);
    const [failed, setFailed] = useState(false);
    useEffect(() => {
      let active = true;
      setFailed(false);
      void load()
        .then((component) => {
          if (active) setPage(() => component);
        })
        .catch(() => {
          if (active) setFailed(true);
        });
      return () => {
        active = false;
      };
    }, []);
    if (Page) return <Page {...props} />;
    // Browsers cache failed module fetches for the document. Retrying the same
    // import cannot reliably recover; an explicit reload creates a new module map.
    if (failed)
      return (
        <section role="alert" className="suna-page-load-failed">
          <p>页面加载失败。重新加载当前页面会关闭未保存的输入。</p>
          <button type="button" onClick={reload}>重新加载页面</button>
        </section>
      );
    return (
      <div className="suna-page-skeleton" role="status" aria-live="polite">
        <div className="suna-skeleton-header">
          <div className="suna-skeleton-spinner" />
          <span className="suna-skeleton-badge">正在加载页面…</span>
        </div>
        <div className="suna-skeleton-body">
          <div className="suna-skeleton-line is-title" />
          <div className="suna-skeleton-line is-sub" />
          <div className="suna-skeleton-grid">
            <div className="suna-skeleton-card" />
            <div className="suna-skeleton-card" />
            <div className="suna-skeleton-card" />
          </div>
        </div>
      </div>
    );
  };
  DeferredPage.preload = load;
  return DeferredPage;
}
