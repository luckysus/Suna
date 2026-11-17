/** Stabilize incomplete markdown markers while an answer is still streaming. */
export function normalizeStreamingMarkdown(text: string) {
  let normalized = text
    .replace(/(^|\n)[ \t]{0,3}(?:(?:-[ \t]*){3,}|(?:\*[ \t]*){3,}|(?:_[ \t]*){3,})$/u, "$1")
    .replace(/(^|\n)[ \t]*(?:[-*+]|[-=]{2,}|\d{1,9}[.)]?)[ \t]*$/u, "$1");

  const withoutClosedCode = normalized.replace(/```[\s\S]*?```|`[^`\n]*`/g, "");
  const trailingMarker = withoutClosedCode.match(/[*~`]+[ \t]*$/u);
  if (trailingMarker) normalized = normalized.slice(0, normalized.length - trailingMarker[0].length);

  const masked = normalized.replace(/```[\s\S]*?```|`[^`\n]*`/g, "");
  const closers: string[] = [];
  if ((masked.match(/~~/g)?.length ?? 0) % 2 === 1) closers.push("~~");
  const boldCount = masked.match(/\*\*/g)?.length ?? 0;
  if (boldCount % 2 === 1) closers.push("**");
  else if (((masked.replace(/\*\*/g, "").match(/\*/g)?.length ?? 0) % 2) === 1) closers.push("*");
  if ((masked.match(/`/g)?.length ?? 0) % 2 === 1) closers.push("`");
  if (closers.length) {
    const trailingWhitespace = normalized.match(/\s+$/u)?.[0] ?? "";
    const cut = normalized.length - trailingWhitespace.length;
    normalized = `${normalized.slice(0, cut)}${closers.join("")}${trailingWhitespace}`;
  }
  return normalized;
}
