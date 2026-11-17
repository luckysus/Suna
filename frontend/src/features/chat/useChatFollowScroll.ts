import { useCallback, useEffect, useRef, useState, type RefObject } from "react";

function nearBottom(element: HTMLElement, threshold = 48) {
  return element.scrollHeight - element.scrollTop - element.clientHeight <= threshold;
}

/** Follow the latest messages unless the user scrolls up. */
export function useChatFollowScroll(
  containerRef: RefObject<HTMLDivElement | null>,
  contentVersion: unknown,
  enabled = true,
) {
  const followingRef = useRef(true);
  const userScrollingRef = useRef(false);
  const [isFollowingLatest, setIsFollowingLatest] = useState(true);

  const followLatest = useCallback(() => {
    followingRef.current = true;
    setIsFollowingLatest(true);
    const element = containerRef.current;
    if (!element) return;
    element.scrollTop = element.scrollHeight;
  }, [containerRef]);

  useEffect(() => {
    const element = containerRef.current;
    if (!element || !enabled) return;

    const onScroll = () => {
      if (!userScrollingRef.current) return;
      const following = nearBottom(element);
      followingRef.current = following;
      setIsFollowingLatest(following);
    };
    const markUser = () => { userScrollingRef.current = true; };
    const onWheel = (event: WheelEvent) => {
      userScrollingRef.current = true;
      if (event.deltaY < 0) {
        followingRef.current = false;
        setIsFollowingLatest(false);
      }
    };

    element.addEventListener("scroll", onScroll, { passive: true });
    element.addEventListener("wheel", onWheel, { passive: true });
    element.addEventListener("touchmove", markUser, { passive: true });
    element.addEventListener("pointerdown", markUser, { passive: true });
    return () => {
      element.removeEventListener("scroll", onScroll);
      element.removeEventListener("wheel", onWheel);
      element.removeEventListener("touchmove", markUser);
      element.removeEventListener("pointerdown", markUser);
    };
  }, [containerRef, enabled]);

  useEffect(() => {
    if (!enabled || !followingRef.current) return;
    const element = containerRef.current;
    if (!element) return;
    element.scrollTop = element.scrollHeight;
  }, [containerRef, contentVersion, enabled]);

  return { isFollowingLatest, followLatest };
}
