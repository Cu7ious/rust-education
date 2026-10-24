"use client";

import { useEffect, useState } from "react";

const VISIBLE_MS = 4000;

export function SlideToast({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  const [leaving, setLeaving] = useState(false);

  useEffect(() => {
    const hide = window.setTimeout(() => setLeaving(true), VISIBLE_MS);
    const done = window.setTimeout(onDismiss, VISIBLE_MS + 350);
    return () => {
      window.clearTimeout(hide);
      window.clearTimeout(done);
    };
  }, [onDismiss]);

  return (
    <div
      role="status"
      aria-live="polite"
      className={`fixed top-4 right-4 z-50 w-[min(22rem,calc(100vw-2rem))] rounded-2xl border bg-popover px-4 py-3 text-sm text-popover-foreground shadow-lg ${leaving ? "toast-out" : "toast-in"}`}
    >
      {message}
    </div>
  );
}
