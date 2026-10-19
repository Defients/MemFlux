import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { AppSettings, TelemetryFrame } from "../types";
import { MiniHud } from "./MiniHud";

const mocks = vi.hoisted(() => ({ frame: null as unknown, monitoring: (_value: boolean) => {}, settings: (_value: unknown) => {}, disable: vi.fn() }));
vi.mock("../lib/native", () => ({ native: {
  latestFrame: async () => mocks.frame,
  settings: async () => ({ settings: { miniHudPreset: "detailed" } }),
  runtimeStatus: async () => ({ paused: false }),
  onFrame: async () => () => {},
  onMonitoring: async (fn: (value: boolean) => void) => { mocks.monitoring = fn; return () => {}; },
  onSettings: async (fn: (value: unknown) => void) => { mocks.settings = fn; return () => {}; },
  disableMiniHud: mocks.disable,
} }));

beforeEach(() => {
  vi.useFakeTimers(); vi.setSystemTime(10_000); mocks.disable.mockResolvedValue(undefined);
  const metric = (value: number) => ({ value, status: "available", timestampMs: 10_000, sampleIntervalMs: 1000 });
  mocks.frame = { snapshot: { timestampMs: 10_000, sampleIntervalMs: 1000,
    totalPhysicalBytes: metric(1000), availablePhysicalBytes: metric(400) },
    cpuUtilization: metric(42), gpuTelemetry: { utilization: metric(50) },
    fpsCapture: { fps: metric(60), frameTimeMs: metric(16.6), frameTimeP99Ms: 20 },
    cleanerStatus: { label: "Configured" } } as unknown as TelemetryFrame;
});
afterEach(() => { cleanup(); vi.useRealTimers(); vi.clearAllMocks(); });

test("pause and silent interruption remove cached measurements and p99", async () => {
  await act(async () => { render(<MiniHud />); });
  expect(screen.getByText("42%")).toBeInTheDocument();
  expect(screen.getByText("20.0 ms")).toBeInTheDocument();
  act(() => mocks.monitoring(false));
  expect(screen.queryByText("42%")).not.toBeInTheDocument();
  expect(screen.queryByText("20.0 ms")).not.toBeInTheDocument();
  act(() => { mocks.monitoring(true); vi.advanceTimersByTime(6000); });
  expect(screen.queryByText("42%")).not.toBeInTheDocument();
  expect(screen.getByText("Paused / awaiting fresh data")).toBeInTheDocument();
});

test("closing patches only native HUD enabled state and live preferences refresh", async () => {
  await act(async () => { render(<MiniHud />); });
  act(() => mocks.settings({ miniHudPreset: "minimal" } as AppSettings));
  expect(screen.queryByText("GPU")).not.toBeInTheDocument();
  fireEvent.click(screen.getByLabelText("Close Mini HUD"));
  expect(mocks.disable).toHaveBeenCalledExactlyOnceWith();
});

test("a stale frame source is hidden even with a fresh memory snapshot", async () => {
  const frame = mocks.frame as TelemetryFrame;
  frame.fpsCapture!.frameTimeMs.timestampMs = 1000;
  frame.fpsCapture!.fps.status = "unavailable";
  await act(async () => { render(<MiniHud />); });
  expect(screen.queryByText("60")).not.toBeInTheDocument();
  expect(screen.queryByText("20.0 ms")).not.toBeInTheDocument();
  expect(screen.getByText("42%")).toBeInTheDocument();
});
