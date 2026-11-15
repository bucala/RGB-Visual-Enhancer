/**
 * Rate-limits a stream of values to one request in flight per key, always
 * delivering the most recent value. Drag a colour picker and the hardware sees
 * every value it can keep up with — never a backlog, and never a stale final
 * colour. (The Rust side coalesces too; this keeps IPC traffic low.)
 */
export function createLatestSender<T>(
  send: (key: string, value: T) => Promise<unknown>,
  onError?: (key: string, error: unknown) => void,
) {
  const pending = new Map<string, T>();
  const inFlight = new Set<string>();

  const pump = async (key: string) => {
    if (inFlight.has(key)) return;
    while (pending.has(key)) {
      const value = pending.get(key) as T;
      pending.delete(key);
      inFlight.add(key);
      try {
        await send(key, value);
      } catch (e) {
        onError?.(key, e);
      } finally {
        inFlight.delete(key);
      }
    }
  };

  return (key: string, value: T) => {
    pending.set(key, value);
    void pump(key);
  };
}
