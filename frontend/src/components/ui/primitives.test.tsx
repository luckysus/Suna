import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Button } from "./button";
import { Card } from "./card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./table";

/**
 * 第 88 章：共享组件的变体必须与既有 `suna-*` 样式类一一对应，
 * 否则「把原生标签换成组件」就会改变视觉。
 */

describe("Button", () => {
  it.each([
    ["primary", "suna-primary-button"],
    ["secondary", "suna-secondary-button"],
    ["ghost", "suna-ghost-button"],
    ["danger", "suna-danger-button"],
    ["plain", "suna-button"],
    ["action", "suna-action-primary"],
    ["actionSecondary", "suna-action-secondary"],
  ] as const)("variant %s renders %s", (variant, className) => {
    const { container } = render(<Button variant={variant}>操作</Button>);
    const button = container.querySelector("button");
    expect(button?.className.split(/\s+/)).toContain(className);
  });

  it("maps the soft variant to both classes", () => {
    const { container } = render(<Button variant="soft">刷新</Button>);
    const classes = container.querySelector("button")?.className.split(/\s+/) ?? [];
    expect(classes).toContain("suna-button");
    expect(classes).toContain("is-soft");
  });

  it("size=icon adds the square icon class without a variant class", () => {
    const { container } = render(<Button size="icon" aria-label="编辑" />);
    const classes = container.querySelector("button")?.className.split(/\s+/) ?? [];
    expect(classes).toContain("suna-icon-button");
    expect(classes).toHaveLength(1);
  });

  it("keeps caller classes and native props", () => {
    const { container } = render(<Button variant="ghost" className="is-active" type="submit" disabled />);
    const button = container.querySelector("button");
    expect(button?.className.split(/\s+/)).toEqual(expect.arrayContaining(["suna-ghost-button", "is-active"]));
    expect(button?.getAttribute("type")).toBe("submit");
    expect(button?.hasAttribute("disabled")).toBe(true);
  });
});

describe("Card", () => {
  it.each([
    ["settings", "suna-settings-card"],
    ["management", "suna-management-card"],
    ["mcp", "suna-mcp-card"],
    ["diagnostics", "suna-diagnostics-card"],
    ["stat", "suna-stat-card"],
    ["registry", "suna-registry-card"],
    ["plan", "suna-plan-card"],
    ["library", "suna-knowledge-library-card"],
  ] as const)("variant %s renders %s", (variant, className) => {
    const { container } = render(<Card variant={variant}>内容</Card>);
    const card = container.firstElementChild;
    expect(card?.className.split(/\s+/)).toContain(className);
  });

  it("renders the requested semantic tag", () => {
    const { container } = render(<Card variant="management" as="article">内容</Card>);
    expect(container.querySelector("article")).not.toBeNull();
    expect(container.querySelector("div")).toBeNull();
  });

  it("keeps extra classes alongside the variant", () => {
    const { container } = render(<Card variant="registry" className="suna-registry-card" />);
    const classes = container.firstElementChild?.className.split(/\s+/) ?? [];
    expect(classes.filter((name) => name === "suna-registry-card")).toHaveLength(2);
  });
});

describe("Table", () => {
  it("renders semantic table elements and puts the wrapper class on the scroll container", () => {
    const { container } = render(
      <Table wrapperClassName="suna-knowledge-table-wrap">
        <TableHeader>
          <TableRow>
            <TableHead>名称</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow>
            <TableCell>Q690</TableCell>
          </TableRow>
        </TableBody>
      </Table>,
    );
    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper.tagName).toBe("DIV");
    expect(wrapper.className.split(/\s+/)).toContain("suna-knowledge-table-wrap");
    expect(container.querySelector("table")).not.toBeNull();
    expect(container.querySelector("thead th")?.textContent).toBe("名称");
    expect(container.querySelector("tbody td")?.textContent).toBe("Q690");
  });

  it("does not impose its own visual classes", () => {
    const { container } = render(
      <Table>
        <TableBody>
          <TableRow>
            <TableCell>值</TableCell>
          </TableRow>
        </TableBody>
      </Table>,
    );
    expect(container.querySelector("table")?.className).toBe("");
    expect(container.querySelector("td")?.className).toBe("");
  });
});
