import { useCallback, useEffect, useRef, useState } from "react";
import type { ActionController } from "../flow/context";

export function useAction<E = unknown>(
  task: () => Promise<void>,
  classify: (error: unknown) => E = (error) => error as E,
): ActionController<E> {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<E | null>(null);
  const latest = useRef({ task, classify });
  const running = useRef(false);
  useEffect(() => {
    latest.current = { task, classify };
  });

  const run = useCallback(() => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError(null);
    latest.current.task().then(
      () => {
        running.current = false;
        setBusy(false);
      },
      (thrown: unknown) => {
        running.current = false;
        setBusy(false);
        setError(latest.current.classify(thrown));
      },
    );
  }, []);

  return { busy, error, run };
}
