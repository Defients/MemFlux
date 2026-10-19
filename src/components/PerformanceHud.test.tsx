import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { PerformanceHud } from "./PerformanceHud";
import type { TelemetryFrame } from "../types";

afterEach(cleanup);

const fixture = () =>
  ({
    snapshot: {
      timestampMs: Date.now(),
      sampleIntervalMs: 1000,
      physicalLoadPercent: { value: 57 }
    },
    cpuUtilization: { value: 34, source: "GetSystemTimes", detail: null },
    gpuTelemetry: {
      utilization: {
        id: "gpu_utilization_percent",
        value: 48,
        unit: "percent",
        source: "WDDM GPU Engine (NVIDIA GeForce RTX 4060 Ti: Busiest engine 3D)",
        timestampMs: Date.now(),
        sampleIntervalMs: 1000,
        status: "available",
        detail: "NVIDIA GeForce RTX 4060 Ti — Busiest engine: 3D (48.0%)"
      },
      selectedAdapter: {
        luidKey: "0x00000000_0x00017a11",
        name: "NVIDIA GeForce RTX 4060 Ti",
        vendorId: 0x10de,
        deviceId: 0x2805,
        dedicatedVideoMemoryBytes: 8589934592,
        sharedSystemMemoryBytes: 17179869184,
        isHardware: true
      },
      availableAdapters: [],
      engineBreakdown: {
        busiestEngineType: "3D",
        threeDPercent: 48,
        computePercent: 0,
        copyPercent: 3,
        videoDecodePercent: 0,
        videoEncodePercent: 0,
        otherPercent: 0
      }
    },
    cleanerDecision: "authorization_required",
    cleanerStatus: {
      state: "configured_unauthorized",
      label: "Configured — not authorized",
      decision: "authorization_required",
      authorizationReady: false,
      sessionOverride: "keep_global",
      purgesLastHour: 0,
      maximumPurgesPerHour: 2,
      cooldownRemainingMs: 0,
      minimumIntervalRemainingMs: 0,
      pollIntervalMs: 5000,
      lastEvaluationMs: Date.now(),
      detail: "Awaiting session authorization"
    }
  }) as unknown as TelemetryFrame;

test("renders measured CPU/GPU/RAM and absent capture without fabricated zero", () => {
  render(<PerformanceHud frame={fixture()} paused={false} />);
  expect(screen.getByLabelText("CPU 34 percent")).toBeInTheDocument();
  expect(screen.getByLabelText("GPU 48 percent")).toBeInTheDocument();
  expect(screen.getByLabelText("RAM 57 percent")).toBeInTheDocument();
  expect(screen.getByLabelText("FPS unavailable")).toHaveTextContent("—");
  expect(screen.getByLabelText("Frame time unavailable")).toHaveTextContent("—");
  expect(screen.getByText("No FPS capture active")).toBeInTheDocument();
  expect(screen.getByText("Cleaner: Configured — not authorized")).toBeInTheDocument();
});

test("paused and old frames do not look live", () => {
  const { rerender } = render(<PerformanceHud frame={fixture()} paused />);
  expect(screen.getByLabelText("CPU unavailable")).toBeInTheDocument();
  expect(screen.getByLabelText("GPU unavailable")).toBeInTheDocument();
  const old = fixture();
  old.snapshot.timestampMs -= 60000;
  rerender(<PerformanceHud frame={old} paused={false} />);
  expect(screen.getByLabelText("RAM unavailable")).toBeInTheDocument();
});

test("respects HUD per-metric customization toggles", () => {
  render(
    <PerformanceHud
      frame={fixture()}
      paused={false}
      settings={{ hudShowGpu: false, hudShowFps: false, hudDetailLevel: "detailed" }}
    />
  );
  expect(screen.getByLabelText("CPU 34 percent")).toBeInTheDocument();
  expect(screen.queryByLabelText("GPU 48 percent")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("FPS unavailable")).not.toBeInTheDocument();
  expect(screen.getByText(/NVIDIA GeForce RTX 4060 Ti/)).toBeInTheDocument();
});
