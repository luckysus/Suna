import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { LocaleProvider } from "../../i18n/locale";
import ChatThoughtPanel from "./ChatThoughtPanel";

function renderThought(props: {
  reasoning: string;
  reasoningMs?: number | null;
  streaming?: boolean;
}) {
  return render(
    <LocaleProvider>
      <ChatThoughtPanel {...props} />
    </LocaleProvider>,
  );
}

describe("ChatThoughtPanel", () => {
  it("renders nothing without reasoning text", () => {
    const { container } = renderThought({ reasoning: "   " });
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a streaming thought open by default", () => {
    renderThought({ reasoning: "先查标准再回答", streaming: true });
    expect(screen.getByText("正在思考")).toBeInTheDocument();
    expect(screen.getByText("先查标准再回答")).toBeInTheDocument();
  });

  it("collapses completed thoughts and expands on click", () => {
    renderThought({ reasoning: "已完成的推理", reasoningMs: 1200, streaming: false });
    expect(screen.getByText("已思考（用时 1 秒）")).toBeInTheDocument();
    expect(screen.queryByText("已完成的推理")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "展开本轮思考" }));
    expect(screen.getByText("已完成的推理")).toBeInTheDocument();
  });

  it("still shows duration when completed with a zero-ms stamp", () => {
    renderThought({ reasoning: "短推理", reasoningMs: 0, streaming: false });
    expect(screen.getByText("已思考（用时 1 秒）")).toBeInTheDocument();
  });
});
