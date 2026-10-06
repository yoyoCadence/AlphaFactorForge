import { describe, expect, it } from 'vitest';
import { createFrameThrottle, type FrameScheduler } from './frameThrottle';

// A manual frame clock: frames run only when the test calls `tick()`.
function manualFrames(): FrameScheduler & { tick(): void; pending(): number } {
  let next = 1;
  const callbacks = new Map<number, () => void>();
  return {
    request(callback) {
      const handle = next++;
      callbacks.set(handle, callback);
      return handle;
    },
    cancel(handle) {
      callbacks.delete(handle);
    },
    tick() {
      const due = [...callbacks.values()];
      callbacks.clear();
      for (const callback of due) callback();
    },
    pending: () => callbacks.size,
  };
}

describe('createFrameThrottle', () => {
  it('runs once per frame with the latest value', () => {
    const frames = manualFrames();
    const seen: number[] = [];
    const throttle = createFrameThrottle<number>((v) => seen.push(v), frames);
    throttle.schedule(1);
    throttle.schedule(2);
    throttle.schedule(3);
    expect(frames.pending()).toBe(1);
    expect(seen).toEqual([]);
    frames.tick();
    expect(seen).toEqual([3]);
    // A new value after the frame asks for a new frame.
    throttle.schedule(4);
    frames.tick();
    expect(seen).toEqual([3, 4]);
    frames.tick();
    expect(seen).toEqual([3, 4]);
  });

  it('flush runs the pending value synchronously and drops its frame', () => {
    const frames = manualFrames();
    const seen: number[] = [];
    const throttle = createFrameThrottle<number>((v) => seen.push(v), frames);
    throttle.schedule(7);
    throttle.flush();
    expect(seen).toEqual([7]);
    expect(frames.pending()).toBe(0);
    frames.tick();
    throttle.flush();
    expect(seen).toEqual([7]);
  });

  it('cancel drops the pending value without running it', () => {
    const frames = manualFrames();
    const seen: number[] = [];
    const throttle = createFrameThrottle<number>((v) => seen.push(v), frames);
    throttle.schedule(9);
    throttle.cancel();
    expect(frames.pending()).toBe(0);
    frames.tick();
    throttle.flush();
    expect(seen).toEqual([]);
    // Still usable afterwards (an effect cleanup may cancel and re-run).
    throttle.schedule(10);
    frames.tick();
    expect(seen).toEqual([10]);
  });
});
