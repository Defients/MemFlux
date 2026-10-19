import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { PerformanceHud } from "./PerformanceHud";
import type { TelemetryFrame } from "../types";

afterEach(cleanup);
const fixture = () => ({
  snapshot: { timestampMs: Date.now(), sampleIntervalMs: 1000, physicalLoadPercent: { value: 57 } },
  cpuUtilization: { value: 34, source: "GetSystemTimes", detail: null },
  cleanerDecision: "authorization_required"
} as TelemetryFrame);

test("renders measured CPU/RAM and absent capture without fabricated zero", () => {
  render(<PerformanceHud frame={fixture()} paused={false} />);
  expect(screen.getByLabelText("CPU 34 percent")).toBeInTheDocument();
  expect(screen.getByLabelText("RAM 57 percent")).toBeInTheDocument();
  expect(screen.getByLabelText("GPU unavailable")).toHaveTextContent("—");
  expect(screen.getByText("No FPS capture active")).toBeInTheDocument();
});

test("paused and old frames do not look live", () => {
  const { rerender } = render(<PerformanceHud frame={fixture()} paused />);
  expect(screen.getByLabelText("CPU unavailable")).toBeInTheDocument();
  const old = fixture(); old.snapshot.timestampMs -= 60000;
  rerender(<PerformanceHud frame={old} paused={false} />);
  expect(screen.getByLabelText("RAM unavailable")).toBeInTheDocument();
});
