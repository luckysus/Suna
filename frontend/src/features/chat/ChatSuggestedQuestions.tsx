import { Button } from "../../components/ui/button";

/** Chip suggestions under an assistant turn. */
export default function ChatSuggestedQuestions({
  questions,
  disabled = false,
  onSelect,
}: {
  questions: string[];
  disabled?: boolean;
  onSelect: (question: string) => void;
}) {
  if (!questions.length) return null;
  return (
    <div className="suna-chat-suggested-questions" aria-label="建议继续问">
      <strong>建议继续问</strong>
      <div className="suna-chat-permission-actions">
        {questions.map((question) => (
          <Button
            key={question}
            type="button"
            variant="actionSecondary"
            disabled={disabled}
            onClick={() => onSelect(question)}
          >
            {question}
          </Button>
        ))}
      </div>
    </div>
  );
}

/** Lightweight local suggestions when the backend did not send any. */
export function deriveSuggestedQuestions(answer: string, limit = 3): string[] {
  const text = answer.trim();
  if (text.length < 24) return [];
  const seeds = [
    "请用更通俗的话解释一下",
    "给出可执行的下一步建议",
    "有哪些风险或注意事项？",
  ];
  if (/标准|牌号|材料|钢/.test(text)) {
    seeds.unshift("相关标准条款有哪些？", "典型成分与力学性能范围是多少？");
  }
  return [...new Set(seeds)].slice(0, limit);
}
