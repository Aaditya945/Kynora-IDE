type PerfValue = string | number | boolean | null | undefined;

type PerfRecord = Record<string, PerfValue>;

const LOG_PREFIX = "[KYNORA/PERF]";

function isBrowser(): boolean {
  return typeof window !== "undefined";
}

function envFlag(): boolean {
  if (!isBrowser()) {
    return false;
  }

  try {
    const globalFlag = (window as Window & {
      __KYNORA_PERF_LOG__?: boolean | string;
    }).__KYNORA_PERF_LOG__;

    if (typeof globalFlag === "string") {
      return ["1", "true", "yes"].includes(globalFlag.toLowerCase());
    }

    if (typeof globalFlag === "boolean") {
      return globalFlag;
    }

    const stored = window.localStorage.getItem("kynora_perf_log");
    if (stored) {
      return ["1", "true", "yes"].includes(stored.toLowerCase());
    }
  } catch {
    // ignored intentionally: storage can be blocked
  }

  return false;
}

function nowMs(): number {
  if (typeof performance !== "undefined" && typeof performance.now === "function") {
    return performance.now();
  }

  return Date.now();
}

function serializeRecord(record: PerfRecord): PerfRecord {
  const output: PerfRecord = {};

  for (const [key, value] of Object.entries(record)) {
    if (value === undefined) {
      continue;
    }

    output[key] = value;
  }

  return output;
}

const logger = {
  isEnabled(): boolean {
    return envFlag();
  },

  setEnabled(value: boolean): void {
    if (!isBrowser()) {
      return;
    }

    try {
      (window as Window & { __KYNORA_PERF_LOG__?: boolean }).__KYNORA_PERF_LOG__ = value;
      window.localStorage.setItem("kynora_perf_log", value ? "1" : "0");
    } catch {
      // ignored intentionally: browser storage may be unavailable
    }
  },

  mark(name: string, extra: PerfRecord = {}): number {
    const startedAt = nowMs();

    if (!this.isEnabled()) {
      return startedAt;
    }

    console.info(LOG_PREFIX, {
      event: `${name}.start`,
      ts: Date.now(),
      monotonic_ms: Number(startedAt.toFixed(2)),
      ...serializeRecord(extra),
    });

    return startedAt;
  },

  event(name: string, extra: PerfRecord = {}): void {
    if (!this.isEnabled()) {
      return;
    }

    console.info(LOG_PREFIX, {
      event: name,
      ts: Date.now(),
      monotonic_ms: Number(nowMs().toFixed(2)),
      ...serializeRecord(extra),
    });
  },

  measure(name: string, start: number, extra: PerfRecord = {}): number {
    const elapsed = nowMs() - start;

    if (!this.isEnabled()) {
      return elapsed;
    }

    console.info(LOG_PREFIX, {
      event: name,
      ts: Date.now(),
      monotonic_ms: Number(nowMs().toFixed(2)),
      duration_ms: Number(elapsed.toFixed(2)),
      ...serializeRecord(extra),
    });

    return elapsed;
  },
};

export const perfLogger = logger;

export function enablePerformanceLogging(nextValue: boolean): void {
  perfLogger.setEnabled(nextValue);
}

export function withPerfTiming<T>(
  name: string,
  work: () => T | Promise<T>,
  extra: PerfRecord = {},
): T | Promise<T> {
  const startedAt = perfLogger.mark(`${name}.start`, extra);

  try {
    const result = work();

    if (result instanceof Promise) {
      return result.finally(() => {
        perfLogger.measure(name, startedAt, extra);
      });
    }

    perfLogger.measure(name, startedAt, extra);
    return result;
  } catch (error) {
    perfLogger.measure(name, startedAt, {
      ...extra,
      failed: true,
      error: error instanceof Error ? error.message : String(error),
    });

    throw error;
  }
}

export const perfEnabled = () => logger.isEnabled();
