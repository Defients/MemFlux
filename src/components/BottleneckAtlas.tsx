import { useEffect, useState } from "react";
import type { AtlasFinding, SensorReading, TelemetryFrame } from "../types";
import { formatBytes, formatTimestamp } from "../lib/format";
import { frameFresh, freshValue } from "../lib/freshness";

function readingValue(reading: SensorReading) {
  if (reading.status !== "available" || reading.value === null) return "—";
  return reading.unit === "bytes" ? formatBytes(reading.value) : `${reading.value.toFixed(1)} ${reading.unit}`;
}

function Evidence({ readings }: { readings: SensorReading[] }) {
  return <div className="sensor-table-wrap"><table className="sensor-table">
    <thead><tr><th>Sensor / source</th><th>Observation</th><th>Attribution & timing</th></tr></thead>
    <tbody>{readings.map((reading, i) => <tr key={`${reading.sensorId}-${reading.eventTimestampMs}-${i}`}>
      <td><strong>{reading.sensorId.replaceAll("_", " ")}</strong><small>{reading.source}</small></td>
      <td>{readingValue(reading)}<small>{reading.status.replaceAll("_", " ")}</small></td>
      <td><details><summary>{reading.targetIdentity}{reading.deviceIdentity ? ` · ${reading.deviceIdentity}` : ""}</summary>
        <p>Event/sample: {reading.eventTimestampMs === null ? "unavailable" : formatTimestamp(reading.eventTimestampMs)}. Observed: {formatTimestamp(reading.observationTimestampMs)}.</p>
        <p>Cadence / coarse time uncertainty: {reading.samplingIntervalMs} / {reading.uncertaintyMs} ms.</p>
        <p>{reading.limitation}</p>
      </details></td>
    </tr>)}</tbody>
  </table></div>;
}

export function BottleneckAtlas({ frame, paused, proofActive, onExperiment }: {
  frame: TelemetryFrame; paused: boolean; proofActive: boolean; onExperiment: (id: string) => void;
}) {
  const [now, setNow] = useState(Date.now());
  const currentTime = Math.max(now, Date.now());
  const [incidentId, setIncidentId] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(false);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  const report = frame.atlas;
  if (!report) return <section className="panel sensor-panel">Awaiting native Atlas state.</section>;
  const fresh = frameFresh(frame, paused, currentTime);
  const incident = report.incidents.find((item) => item.id === incidentId);
  const finding: AtlasFinding | undefined = incident?.finding ?? report.findings[0];
  const fps = fresh ? freshValue(frame.fpsCapture?.fps, currentTime) : null;
  const target = frame.fpsCapture?.targetExecutable ?? frame.coordinator?.gamefocus.activeExecutableName ?? "System observation";
  return <section className="panel sensor-panel atlas-panel" aria-label="Bottleneck Atlas">
    <div className="panel-heading"><div><span className="eyebrow">CosmoTech™ · Diagnostic alpha</span><h2>Bottleneck Atlas</h2></div>
      <button className="button button-quiet" aria-pressed={advanced} onClick={() => setAdvanced(!advanced)}>{advanced ? "Simple view" : "Advanced evidence"}</button>
    </div>
    <p className="atlas-summary">{fresh ? report.summary : "Investigation paused or stale. Awaiting fresh native measurements."}</p>
    <p className="sensor-caption">{target} · {fps === null ? "FPS unavailable" : `${fps.toFixed(0)} measured FPS`} · {report.retainedSamples}/90 observation ticks · No diagnostic trace started by Atlas</p>
    <div className="atlas-domains" aria-label="Measurement coverage">
      {report.domains.map((domain) => <details key={domain.domain} className="atlas-domain"><summary><strong>{domain.domain}</strong><span>{domain.status.replaceAll("_", " ")}</span></summary><p>{domain.detail}</p></details>)}
    </div>
    {fresh && finding && <div className="atlas-finding">
      <div className="panel-heading"><h3>{finding.title}</h3><span className="availability-tag tag-warming_up">{finding.confidence.replaceAll("_", " ")} evidence</span></div>
      <p>{finding.explanation}</p>
      <details><summary>Evidence, gaps & competing observations</summary>
        {finding.evidence.length > 0 && <Evidence readings={finding.evidence} />}
        <ul>{finding.missingEvidence.map((gap) => <li key={gap}>{gap}</li>)}{finding.contradictoryEvidence.map((gap) => <li key={gap}>{gap}</li>)}</ul>
      </details>
      <div className="atlas-experiment"><strong>Focused next test</strong><p>{finding.recommendedTest}</p>
        <button className="button button-primary" disabled={proofActive} onClick={() => onExperiment(`${incident?.id ?? "system"}:${finding.id}`)}>Begin linked Proof baseline</button>
        <small>Records a baseline only. Choose any intervention yourself and mark its boundary in Performance Proof below. Utilization changes alone cannot establish a benefit.</small>
      </div>
    </div>}
    <div className="atlas-timeline"><h3>Incident investigations</h3>
      <label>Surrounding evidence window <select aria-label="Incident investigation" value={incident?.id ?? ""} onChange={(e) => setIncidentId(e.target.value || null)}>
        <option value="">Current system observations</option>
        {report.incidents.slice().reverse().map((item) => <option key={item.id} value={item.id}>{item.eventTimestampMs === null ? "Event time unavailable" : formatTimestamp(item.eventTimestampMs)} · {item.frameTimeMs.toFixed(1)} ms · {item.severity}</option>)}
      </select></label>
      {report.incidents.length === 0 && <p>No incidents retained. Explicit frame capture is required; unavailable data is never synthesized.</p>}
      {incident && <p>{incident.targetIdentity} · {incident.contextSamples} coarse samples · {incident.contextState.replaceAll("_", " ")} · ±3 second context window. Arrival: {formatTimestamp(incident.observationTimestampMs)}. Coincidence does not establish causation.</p>}
      {advanced && incident && <p>{incident.provider} · QPC: {incident.providerQpc ?? "unavailable"} · provider seconds: {incident.providerSeconds ?? "unavailable"}. Event-to-UTC mapping rejects &gt;100 ms clock drift; end-to-end precision remains unverified in gameplay.</p>}
    </div>
    {advanced && <details open><summary>Recorded sensor registry · {formatTimestamp(report.timestampMs)} · {report.evaluationMicros} µs engine evaluation</summary><Evidence readings={report.sensors} /></details>}
  </section>;
}
