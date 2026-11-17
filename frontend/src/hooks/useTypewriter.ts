import { useEffect, useMemo, useRef, useState } from "react";

export interface TypewriterOptions {
  minCps?: number;
  drainSeconds?: number;
  maxCps?: number;
  maxFrameSeconds?: number;
}

const NATURAL_BREAK_RE = /[\s，。！？；：、,.!?;:)\]】》」』]/u;
const CJK_RE = /[\u3400-\u9fff\uf900-\ufaff\u3040-\u30ff\uac00-\ud7af]/u;
const WORD_CHARACTER_RE = /[\p{L}\p{N}_]/u;

function nextCodePointEnd(text: string, index: number) {
  if (index >= text.length) return text.length;
  const code = text.charCodeAt(index);
  return index + (code >= 0xd800 && code <= 0xdbff ? 2 : 1);
}

function previousCodePointStart(text: string, index: number) {
  if (index <= 0) return 0;
  const code = text.charCodeAt(index - 1);
  return code >= 0xdc00 && code <= 0xdfff ? index - 2 : index - 1;
}

function advanceCodePoints(text: string, start: number, count: number) {
  let end = start;
  for (let i = 0; i < count && end < text.length; i += 1) end = nextCodePointEnd(text, end);
  return end;
}

function commonPrefixLength(left: string, right: string, limit: number) {
  let index = 0;
  const max = Math.min(limit, left.length, right.length);
  while (index < max && left.charCodeAt(index) === right.charCodeAt(index)) index += 1;
  return index;
}

function nextRevealBoundary(text: string, start: number, availableCharacters: number) {
  if (start >= text.length) return text.length;
  const firstEnd = nextCodePointEnd(text, start);
  const first = text.slice(start, firstEnd);
  const isCjk = CJK_RE.test(first);
  const isWord = WORD_CHARACTER_RE.test(first);
  const minimumGroup = isCjk ? 2 : 1;
  const maximumGroup = isCjk ? 4 : 14;
  const available = Math.max(0, Math.floor(availableCharacters));
  if (available < minimumGroup && text.length - start > available) return start;

  const hardEnd = advanceCodePoints(text, start, Math.min(maximumGroup, Math.max(minimumGroup, available)));
  const minimumEnd = advanceCodePoints(text, start, minimumGroup);
  let cursor = hardEnd;
  while (cursor >= minimumEnd) {
    const characterStart = previousCodePointStart(text, cursor);
    if (NATURAL_BREAK_RE.test(text.slice(characterStart, cursor))) return cursor;
    cursor = characterStart;
  }
  if (isCjk || !isWord || hardEnd === text.length || available >= maximumGroup) return hardEnd;
  return start;
}

/** Smooth uneven network chunks into a readable cadence (ported from steel-agent). */
export function useTypewriter(target: string, complete: boolean, options: TypewriterOptions = {}) {
  const minCps = options.minCps ?? 72;
  const drainSeconds = options.drainSeconds ?? 0.42;
  const maxCps = options.maxCps ?? 240;
  const maxFrameSeconds = options.maxFrameSeconds ?? 0.05;
  const [typedLength, setTypedLength] = useState(() => (complete ? target.length : 0));
  const typedLengthRef = useRef(typedLength);
  const targetRef = useRef(target);
  const rafRef = useRef<number | null>(null);
  const lastTimestampRef = useRef(0);
  const revealCreditRef = useRef(0);
  const previousTargetRef = useRef(target);
  const reduceMotionRef = useRef(false);
  const ensureAnimationRef = useRef<(() => void) | null>(null);
  targetRef.current = target;

  const setLength = (value: number) => {
    typedLengthRef.current = value;
    setTypedLength(value);
  };

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    reduceMotionRef.current = query.matches;
    const onChange = (event: MediaQueryListEvent) => {
      reduceMotionRef.current = event.matches;
      if (event.matches) {
        revealCreditRef.current = 0;
        setLength(targetRef.current.length);
      }
    };
    query.addEventListener?.("change", onChange);
    return () => query.removeEventListener?.("change", onChange);
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const stop = () => {
      if (rafRef.current !== null) window.cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
      lastTimestampRef.current = 0;
    };
    const tick = (timestamp: number) => {
      const currentTarget = targetRef.current;
      if (typedLengthRef.current > currentTarget.length) {
        revealCreditRef.current = 0;
        setLength(0);
      }
      if (typedLengthRef.current >= currentTarget.length || reduceMotionRef.current) {
        if (reduceMotionRef.current) setLength(currentTarget.length);
        stop();
        return;
      }
      const dt = lastTimestampRef.current
        ? Math.min((timestamp - lastTimestampRef.current) / 1000, maxFrameSeconds)
        : 0;
      lastTimestampRef.current = timestamp;
      const remaining = currentTarget.length - typedLengthRef.current;
      const cps = Math.min(maxCps, Math.max(minCps, remaining / drainSeconds));
      revealCreditRef.current += cps * dt;
      const next = nextRevealBoundary(currentTarget, typedLengthRef.current, revealCreditRef.current);
      if (next > typedLengthRef.current) {
        revealCreditRef.current = Math.max(0, revealCreditRef.current - (next - typedLengthRef.current));
        setLength(next);
      }
      rafRef.current = window.requestAnimationFrame(tick);
    };
    const ensure = () => {
      if (rafRef.current === null && typedLengthRef.current < targetRef.current.length && !reduceMotionRef.current) {
        lastTimestampRef.current = 0;
        rafRef.current = window.requestAnimationFrame(tick);
      }
    };
    ensureAnimationRef.current = ensure;
    return stop;
  }, [drainSeconds, maxCps, maxFrameSeconds, minCps]);

  useEffect(() => {
    if (complete || reduceMotionRef.current) {
      if (rafRef.current !== null) window.cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
      lastTimestampRef.current = 0;
      revealCreditRef.current = 0;
      setLength(target.length);
      previousTargetRef.current = target;
      return;
    }
    if (previousTargetRef.current !== target && target.length < typedLengthRef.current) {
      revealCreditRef.current = 0;
      setLength(commonPrefixLength(previousTargetRef.current, target, typedLengthRef.current));
    }
    if (typedLengthRef.current < target.length) ensureAnimationRef.current?.();
    previousTargetRef.current = target;
  }, [complete, target]);

  return useMemo(() => {
    if (complete) return target;
    let length = Math.min(typedLength, target.length);
    if (length > 0 && length < target.length && target.charCodeAt(length - 1) >= 0xd800 && target.charCodeAt(length - 1) <= 0xdbff) {
      length -= 1;
    }
    return target.slice(0, length);
  }, [target, typedLength, complete]);
}
