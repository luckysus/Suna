import { useState } from "react";
import { Button } from "../../components/ui/button";
import { Textarea } from "../../components/ui/textarea";

/** Structured clarification form — mirrors steel-agent AgentFollowUpForm. */
export default function ChatFollowUpForm({
  questions,
  disabled = false,
  onSubmit,
}: {
  questions: string[];
  disabled?: boolean;
  onSubmit: (answer: string) => void;
}) {
  const [answers, setAnswers] = useState<Record<number, string>>({});
  if (!questions.length) return null;

  const submit = () => {
    const lines = questions.map((question, index) => {
      const value = (answers[index] ?? "").trim();
      return value ? `${question}\n${value}` : null;
    }).filter((line): line is string => Boolean(line));
    if (!lines.length) return;
    onSubmit(lines.join("\n\n"));
  };

  return (
    <div className="suna-chat-follow-up-form" aria-label="需要补充的信息">
      <strong>需要补充的信息</strong>
      <div className="suna-chat-follow-up-fields">
        {questions.map((question, index) => (
          <label key={`${question}-${index}`} className="suna-chat-follow-up-field">
            <span>{question}</span>
            <Textarea
              rows={2}
              value={answers[index] ?? ""}
              disabled={disabled}
              onChange={(event) => setAnswers((current) => ({ ...current, [index]: event.target.value }))}
              placeholder="请填写"
            />
          </label>
        ))}
      </div>
      <Button type="button" variant="action" disabled={disabled} onClick={submit}>提交补充信息</Button>
    </div>
  );
}
