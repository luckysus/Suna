import type { LucideIcon } from "lucide-react";
import { Command, Search } from "lucide-react";
import type { RefObject } from "react";
import { Input } from "./input";
import type { SectionId } from "../../types/navigation";
import "./command-menu.css";

/**
 * 第 63 章组件规范：Command Menu（命令面板 / 全局搜索）。
 *
 * 纯展示组件：查询状态由父级持有，分组结果由父级组装，
 * 便于第 72 章把「对话 / 知识库 / 文献 / Agent / Skill / MCP / 模型 / 设置」
 * 统一塞进同一个面板。
 */

export type CommandMenuItem = {
  id: string;
  title: string;
  subtitle?: string;
  /** 选中后跳转的模块；不填则由父级自行处理。 */
  section?: SectionId;
  icon?: LucideIcon;
  /** 父级私有负载（如会话 id）。 */
  payload?: unknown;
};

export type CommandMenuGroup = {
  id: string;
  label: string;
  items: CommandMenuItem[];
};

export function CommandMenu({
  open,
  onClose,
  query,
  onQueryChange,
  inputRef,
  groups,
  onSelect,
  placeholder = "搜索对话、知识库、文献、Agent…",
  hint = "搜索对话、知识库、文献、Agent、Skill、MCP、模型与设置",
  emptyText = "没有找到匹配内容",
  ariaLabel = "全局搜索",
}: {
  open: boolean;
  onClose: () => void;
  query: string;
  onQueryChange: (value: string) => void;
  inputRef?: RefObject<HTMLInputElement | null>;
  groups: CommandMenuGroup[];
  onSelect: (item: CommandMenuItem) => void;
  placeholder?: string;
  hint?: string;
  emptyText?: string;
  ariaLabel?: string;
}) {
  if (!open) return null;
  const visible = groups.filter((group) => group.items.length > 0);
  return (
    <div className="suna-new-search-overlay" role="dialog" aria-label={ariaLabel} onClick={onClose}>
      <div className="suna-new-search" onClick={(event) => event.stopPropagation()}>
        <div>
          <Command size={17} aria-hidden="true" />
          <Input
            ref={inputRef}
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder={placeholder}
            aria-label={ariaLabel}
          />
          <kbd>Esc</kbd>
        </div>
        {query.trim()
          ? visible.length > 0
            ? (
              <div className="suna-search-results">
                {visible.map((group) => (
                  <section key={group.id} className="suna-search-group" aria-label={group.label}>
                    <p className="suna-search-group-label">{group.label}</p>
                    {group.items.map((item) => {
                      const Icon = item.icon ?? Search;
                      return (
                        <button key={item.id} onClick={() => onSelect(item)}>
                          <Icon size={15} aria-hidden="true" />
                          <span>
                            <strong>{item.title}</strong>
                            {item.subtitle && <small>{item.subtitle}</small>}
                          </span>
                        </button>
                      );
                    })}
                  </section>
                ))}
              </div>
            )
            : <p className="suna-search-no-results">{emptyText}</p>
          : <p>{hint}</p>}
      </div>
    </div>
  );
}
