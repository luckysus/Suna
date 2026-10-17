import { lazy, Suspense, type ReactNode } from "react";

const AIAnswerRenderer = lazy(() => import("./AnswerRenderer"));

export default function LazyAIAnswerRenderer({
  answer,
  literatureResults,
  fallback,
}: {
  answer: string;
  literatureResults: import("./AnswerRenderer").AnswerReferenceResult[];
  fallback?: ReactNode;
}) {
  return (
    <Suspense fallback={fallback ?? <pre className="suna-chat-plain-preview">{answer}</pre>}>
      <AIAnswerRenderer answer={answer} literatureResults={literatureResults} />
    </Suspense>
  );
}
