import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { TelemetryFrame } from "../types";
import { BottleneckAtlas } from "./BottleneckAtlas";
afterEach(cleanup);

function fixture() {
  const finding = { id: "memory-paging-pressure", title: "Memory/paging pressure near a frame anomaly", confidence: "weak",
    explanation: "System-wide counters cannot establish workload cause.", evidence: [], missingEvidence: ["Game hard faults unavailable"],
    contradictoryEvidence: ["Recovery observed"], recommendedTest: "Repeat an equivalent scene." };
  return { snapshot: { timestampMs: Date.now(), sampleIntervalMs: 1000 },
    atlas: { summary: "No reliable bottleneck identified with the currently available measurements.",
      findings: [finding], sensors: [], domains: [{ domain: "Storage", status: "unavailable", detail: "Latency source missing" }],
      retainedSamples: 3, evaluationMicros: 50, incidents: [{ id: "session:incident", finding,
        eventTimestampMs: null, observationTimestampMs: Date.now(), targetIdentity: "pid:42:created:123", frameTimeMs: 100,
        severity: "Severe", contextSamples: 0, contextState: "unavailable" }] },
  } as unknown as TelemetryFrame;
}
test("explains evidence gaps and starts a linked baseline without an intervention", () => {
  const onExperiment = vi.fn();
  render(<BottleneckAtlas frame={fixture()} paused={false} proofActive={false} onExperiment={onExperiment} />);
  expect(screen.getByText(/No reliable bottleneck/)).toBeInTheDocument();
  expect(screen.getByText("Game hard faults unavailable")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Incident investigation"), { target: { value: "session:incident" } });
  fireEvent.click(screen.getByText("Begin linked Proof baseline"));
  expect(onExperiment).toHaveBeenCalledWith("session:incident:memory-paging-pressure");
  expect(screen.getByText(/Event time unavailable/)).toBeInTheDocument();
});
test("paused and stale reports cannot offer a current experiment", () => {
  const frame = fixture(); const onExperiment = vi.fn();
  const { rerender } = render(<BottleneckAtlas frame={frame} paused proofActive={false} onExperiment={onExperiment} />);
  expect(screen.queryByText("Begin linked Proof baseline")).not.toBeInTheDocument();
  frame.snapshot.timestampMs -= 60_000;
  rerender(<BottleneckAtlas frame={frame} paused={false} proofActive={false} onExperiment={onExperiment} />);
  expect(screen.getByText(/Investigation paused or stale/)).toBeInTheDocument();
});
