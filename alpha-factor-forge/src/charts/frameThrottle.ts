// PERF-CHART-COMPUTE-001 — coalesce high-rate input (chart pointer moves) into
// at most one handler call per animation frame, always with the latest value.
// Pure scheduling logic: the frame source is injected, so the semantics are
// unit-tested without a browser.

export interface FrameScheduler {
  request(callback: () => void): number;
  cancel(handle: number): void;
}

export interface FrameThrottle<T> {
  /** Keep `value` as the latest; run it once on the next frame. */
  schedule(value: T): void;
  /** Run the pending value now, synchronously, and drop its frame. */
  flush(): void;
  /** Drop the pending value and its frame without running it. */
  cancel(): void;
}

export function createFrameThrottle<T>(run: (value: T) => void, scheduler: FrameScheduler): FrameThrottle<T> {
  let pending: { value: T } | null = null;
  let handle: number | null = null;

  const dropFrame = (): void => {
    if (handle != null) scheduler.cancel(handle);
    handle = null;
  };
  const runPending = (): void => {
    const next = pending;
    pending = null;
    if (next) run(next.value);
  };

  return {
    schedule(value) {
      pending = { value };
      if (handle == null) {
        handle = scheduler.request(() => {
          handle = null;
          runPending();
        });
      }
    },
    flush() {
      dropFrame();
      runPending();
    },
    cancel() {
      dropFrame();
      pending = null;
    },
  };
}

/** The browser's animation frames. */
export const animationFrames: FrameScheduler = {
  request: (callback) => requestAnimationFrame(() => callback()),
  cancel: (handle) => cancelAnimationFrame(handle),
};
