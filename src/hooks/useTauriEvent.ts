import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Subscribe to a backend Tauri event for the lifetime of the component.
 * `handler` is called with each event payload. `enabled = false` skips
 * subscription entirely (useful for conditional listeners).
 */
export function useTauriEvent<T>(
  eventName: string,
  handler: (payload: T) => void,
  enabled: boolean = true
) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    if (!enabled) return;
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<T>(eventName, (e) => handlerRef.current(e.payload)).then((u) => {
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [eventName, enabled]);
}
