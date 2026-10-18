import { describe, expect, it } from "vitest";
import { formatBytes, formatPercent, formatRate } from "./format";

describe("telemetry formatting", () => {
  it("formats binary byte units without inventing unavailable values", () => {
    expect(formatBytes(16 * 1024 ** 3)).toBe("16.0 GB");
    expect(formatBytes(null)).toBe("Unavailable");
    expect(formatBytes(-1)).toBe("Unavailable");
  });

  it("preserves honest percent and rate labels", () => {
    expect(formatPercent(41.25, 1)).toBe("41.3%");
    expect(formatRate(3.25)).toBe("3.3 /s");
    expect(formatRate(null)).toBe("Unavailable");
  });
});

