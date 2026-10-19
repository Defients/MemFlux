import { useEffect, useMemo, useState, type CSSProperties, type ReactNode } from "react";
import { useMemFlux } from "./hooks/useMemFlux";
import {
  availablePercent,
  formatBytes,
  formatCount,
  formatMetric,
  formatPercent,
  formatTimestamp,
  pressureLabel,
  statusLabel
} from "./lib/format";
import type {
  ActionCapability,
  ActionRecord,
  AppSettings,
  GuardianDecision,
  MemorySnapshot,
  Metric,
  PageId,
  PressureLevel,
  SettingsEnvelope,
  TelemetryFrame
} from "./types";

const NAVIGATION: Array<{ id: PageId; label: string; caption: string; icon: IconName }> = [
  { id: "overview", label: "Overview", caption: "Live condition", icon: "overview" },
  { id: "explorer", label: "Memory Explorer", caption: "Sensor detail", icon: "explorer" },
  { id: "guardian", label: "Adaptive Guardian", caption: "Decision engine", icon: "guardian" },
  { id: "settings", label: "Settings", caption: "Behavior & display", icon: "settings" }
];

type IconName = "overview" | "explorer" | "guardian" | "settings" | "pause" | "play" | "shield" | "pulse" | "info";

function Icon({ name, size = 20 }: { name: IconName; size?: number }) {
  const paths: Record<IconName, ReactNode> = {
    overview: <><path d="M4 13h6V4H4v9Zm0 7h6v-4H4v4Zm10 0h6v-9h-6v9Zm0-16v4h6V4h-6Z" /></>,
    explorer: <><path d="M4 18V6m5 12V9m5 9V4m5 14v-7" /><path d="M2.5 18.5h19" /></>,
    guardian: <path d="M12 3 4.5 6v5.7c0 4.4 3 7.8 7.5 9.3 4.5-1.5 7.5-4.9 7.5-9.3V6L12 3Zm0 4v10m-4-5h8" />,
    settings: <><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.8 1.8 0 0 0 .4 2l.1.1-2.8 2.8-.1-.1a1.8 1.8 0 0 0-2-.4 1.8 1.8 0 0 0-1 1.6V21h-4v-.1a1.8 1.8 0 0 0-1-1.6 1.8 1.8 0 0 0-2 .4l-.1.1-2.8-2.8.1-.1a1.8 1.8 0 0 0 .4-2A1.8 1.8 0 0 0 3 14H3v-4h.1a1.8 1.8 0 0 0 1.6-1 1.8 1.8 0 0 0-.4-2l-.1-.1L7 4.1l.1.1a1.8 1.8 0 0 0 2 .4A1.8 1.8 0 0 0 10 3V3h4v.1a1.8 1.8 0 0 0 1 1.6 1.8 1.8 0 0 0 2-.4l.1-.1L20 7l-.1.1a1.8 1.8 0 0 0-.4 2 1.8 1.8 0 0 0 1.6 1H21v4h-.1a1.8 1.8 0 0 0-1.5.9Z" /></>,
    pause: <><path d="M8 5v14M16 5v14" /></>,
    play: <path d="m8 5 11 7-11 7V5Z" />,
    shield: <path d="M12 3 4.5 6v5.7c0 4.4 3 7.8 7.5 9.3 4.5-1.5 7.5-4.9 7.5-9.3V6L12 3Z" />,
    pulse: <path d="M3 12h4l2-5 4 10 2-5h6" />,
    info: <><circle cx="12" cy="12" r="9" /><path d="M12 11v6m0-10v.1" /></>
  };
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {paths[name]}
    </svg>
  );
}

function BrandMark() {
  return (
    <svg viewBox="0 0 56 56" className="brand-mark" aria-hidden="true">
      <defs>
        <linearGradient id="brand-gradient" x1="8" y1="6" x2="50" y2="52" gradientUnits="userSpaceOnUse">
          <stop stopColor="#D58FFF" />
          <stop offset=".5" stopColor="#A978FF" />
          <stop offset="1" stopColor="#78BEFF" />
        </linearGradient>
      </defs>
      <rect x="1" y="1" width="54" height="54" rx="15" fill="#18152d" stroke="#3f3560" />
      <path d="M13 38V18c0-2.5 2-4.5 4.5-4.5h1.7L28 29.3l8.8-15.8h1.7c2.5 0 4.5 2 4.5 4.5v20" stroke="url(#brand-gradient)" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M18 42h20" stroke="#f3eeff" strokeWidth="3.5" strokeLinecap="round" opacity=".9" />
    </svg>
  );
}

function StatusPill({ pressure, compact = false }: { pressure: PressureLevel; compact?: boolean }) {
  return (
    <span className={`status-pill status-${pressure} ${compact ? "is-compact" : ""}`}>
      <span className="status-dot" />
      {pressureLabel(pressure)}
    </span>
  );
}

function AppShell({
  page,
  onPage,
  frame,
  paused,
  onPause,
  children
}: {
  page: PageId;
  onPage: (page: PageId) => void;
  frame: TelemetryFrame | null;
  paused: boolean;
  onPause: () => void;
  children: ReactNode;
}) {
  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <BrandMark />
          <div>
            <strong>MEMFLUX</strong>
            <span>by CosmoTech™</span>
          </div>
        </div>

        <nav className="nav-list">
          {NAVIGATION.map((item) => (
            <button
              key={item.id}
              className={`nav-item ${page === item.id ? "is-active" : ""}`}
              onClick={() => onPage(item.id)}
              aria-current={page === item.id ? "page" : undefined}
            >
              <span className="nav-icon"><Icon name={item.icon} /></span>
              <span><strong>{item.label}</strong><small>{item.caption}</small></span>
            </button>
          ))}
        </nav>

        <div className="sidebar-footer">
          <div className="engine-status">
            <span className={`engine-orbit ${paused ? "is-paused" : ""}`}><Icon name="pulse" size={17} /></span>
            <span><strong>{paused ? "Monitoring paused" : "Engine online"}</strong><small>{frame ? `Sample #${frame.snapshot.sequence}` : "Initializing sensors"}</small></span>
          </div>
          <div className="version-row"><span>Cleaner milestone</span><span>v0.2.0</span></div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="breadcrumbs"><span>Memory intelligence</span><b>/</b><strong>{NAVIGATION.find((item) => item.id === page)?.label}</strong></div>
          <div className="topbar-actions">
            {frame && <StatusPill pressure={paused ? "paused" : frame.guardian.pressure} compact />}
            <button className="button button-quiet" onClick={onPause}>
              <Icon name={paused ? "play" : "pause"} size={17} />
              {paused ? "Resume" : "Pause"}
            </button>
          </div>
        </header>
        <div className="page-scroll">{children}</div>
      </main>
    </div>
  );
}

function PageTitle({ eyebrow, title, description, side }: { eyebrow: string; title: string; description: string; side?: ReactNode }) {
  return (
    <div className="page-title">
      <div><span className="eyebrow">{eyebrow}</span><h1>{title}</h1><p>{description}</p></div>
      {side && <div className="page-title-side">{side}</div>}
    </div>
  );
}

function LoadingView() {
  return (
    <div className="loading-view" role="status" aria-live="polite">
      <div className="loading-glyph"><BrandMark /><span /></div>
      <span className="eyebrow">Native telemetry handshake</span>
      <h1>Reading Windows memory state</h1>
      <p>MemFlux is waiting for the first documented API sample. No placeholder values are shown.</p>
      <div className="loading-bar"><span /></div>
    </div>
  );
}

function ErrorBanner({ message }: { message: string }) {
  return <div className="error-banner" role="alert"><Icon name="info" /><div><strong>Runtime connection issue</strong><span>{message}</span></div></div>;
}

function MetricCard({ label, metric, formatter, context, accent }: { label: string; metric: Metric; formatter?: (value: number | null) => string; context: string; accent: string }) {
  const value = metric.status === "available" ? (formatter ? formatter(metric.value) : formatMetric(metric)) : metric.status === "warming_up" ? "Warming up" : "Unavailable";
  return (
    <article className="metric-card" style={{ "--metric-accent": accent } as CSSProperties} title={metric.detail ?? metric.source}>
      <div className="metric-heading"><span>{label}</span><span className={`sensor-dot sensor-${metric.status}`} aria-label={`Sensor ${statusLabel(metric.status)}`} /></div>
      <strong className="metric-value">{value}</strong>
      <p>{context}</p>
      <div className="metric-source"><span>{metric.source}</span><span>{metric.sampleIntervalMs / 1000}s</span></div>
    </article>
  );
}

function MemoryDial({ snapshot }: { snapshot: MemorySnapshot }) {
  const load = snapshot.physicalLoadPercent.value ?? 0;
  const style = { "--dial-value": `${load * 3.6}deg` } as CSSProperties;
  return (
    <div className="memory-dial" style={style} role="progressbar" aria-label="Physical memory load" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(load)}>
      <div className="dial-glow" />
      <div className="dial-core">
        <span>Physical load</span>
        <strong>{formatPercent(snapshot.physicalLoadPercent.value)}</strong>
        <small>{formatBytes(snapshot.availablePhysicalBytes.value)} available</small>
      </div>
    </div>
  );
}

function GuardianPanel({ decision, updated }: { decision: GuardianDecision; updated: number }) {
  return (
    <article className={`guardian-panel guardian-${decision.pressure}`}>
      <div className="guardian-symbol"><Icon name="shield" size={26} /><span /></div>
      <div className="guardian-copy">
        <div className="guardian-heading"><span className="eyebrow">Adaptive Guardian</span><StatusPill pressure={decision.pressure} compact /></div>
        <h2>{decision.outcome === "recommend" ? "Review recommended" : "Observation is the intelligent action"}</h2>
        <p>{decision.explanation}</p>
        <div className="decision-meta"><code>{decision.reasonCode}</code><span>Confidence: {decision.confidence}</span><span>Updated {formatTimestamp(updated)}</span></div>
      </div>
    </article>
  );
}

function MemoryComposition({ snapshot }: { snapshot: MemorySnapshot }) {
  const total = snapshot.totalPhysicalBytes.value;
  const available = snapshot.availablePhysicalBytes.value;
  const nonAvailable = snapshot.nonAvailablePhysicalBytes.value;
  if (total === null || available === null || nonAvailable === null || total <= 0) {
    return <UnavailablePanel title="Memory composition unavailable" detail="Core physical-memory values are not available." />;
  }
  const standby = snapshot.standbyBytes.value;
  const freeZero = snapshot.freeZeroBytes.value;
  const compositionAvailable = standby !== null && freeZero !== null;
  const otherAvailable = compositionAvailable ? Math.max(0, available - standby - freeZero) : available;
  const segments = [
    { key: "active", label: "Non-available", value: nonAvailable, color: "var(--accent-primary)" },
    ...(compositionAvailable ? [
      { key: "standby", label: "Standby", value: standby, color: "var(--accent-secondary)" },
      { key: "free", label: "Free + zero", value: freeZero, color: "var(--status-success)" }
    ] : []),
    { key: "other", label: compositionAvailable ? "Other available" : "Available", value: otherAvailable, color: "var(--surface-hover)" }
  ];
  return (
    <section className="panel composition-panel">
      <div className="panel-heading"><div><span className="eyebrow">Physical memory</span><h2>Composition without double counting</h2></div><span className="sensor-caption">{compositionAvailable ? "All segments are mutually exclusive" : "Detailed counters unavailable"}</span></div>
      <div className="composition-bar" aria-label="Physical memory composition">
        {segments.map((segment) => <span key={segment.key} style={{ width: `${Math.max(0, segment.value / total * 100)}%`, background: segment.color }} title={`${segment.label}: ${formatBytes(segment.value)}`} />)}
      </div>
      <div className="composition-legend">
        {segments.map((segment) => (
          <div key={segment.key}><span className="legend-swatch" style={{ background: segment.color }} /><span><small>{segment.label}</small><strong>{formatBytes(segment.value)}</strong></span></div>
        ))}
      </div>
      <p className="panel-note"><Icon name="info" size={16} /> Available memory already includes standby, free, and zero pages. MemFlux never adds standby twice.</p>
    </section>
  );
}

function TrendGraph({ history }: { history: TelemetryFrame[] }) {
  const width = 680;
  const height = 208;
  const padding = 18;
  const points = history.map((frame, index) => ({
    x: history.length <= 1 ? width / 2 : padding + index / (history.length - 1) * (width - padding * 2),
    available: availablePercent(frame),
    commit: frame.snapshot.commitPercent.value
  }));
  const polyline = (selector: (point: (typeof points)[number]) => number | null) => points
    .filter((point) => selector(point) !== null)
    .map((point) => `${point.x.toFixed(1)},${(padding + (100 - (selector(point) ?? 0)) / 100 * (height - padding * 2)).toFixed(1)}`)
    .join(" ");
  const availableLine = polyline((point) => point.available);
  const commitLine = polyline((point) => point.commit);
  return (
    <section className="panel trend-panel">
      <div className="panel-heading"><div><span className="eyebrow">Live trend</span><h2>Headroom & commit pressure</h2></div><div className="trend-legend"><span className="available-line">Available</span><span className="commit-line">Commit</span></div></div>
      <div className="chart-frame">
        <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label="Available physical memory percentage and commit utilization trend">
          <defs><linearGradient id="area-fill" x1="0" y1="0" x2="0" y2="1"><stop stopColor="#78beff" stopOpacity=".22" /><stop offset="1" stopColor="#78beff" stopOpacity="0" /></linearGradient></defs>
          {[25, 50, 75].map((value) => <g key={value}><line x1={padding} x2={width - padding} y1={padding + (100 - value) / 100 * (height - padding * 2)} y2={padding + (100 - value) / 100 * (height - padding * 2)} className="chart-grid" /><text x={padding + 2} y={padding + (100 - value) / 100 * (height - padding * 2) - 5}>{value}%</text></g>)}
          {availableLine && <polyline points={availableLine} className="chart-line chart-available" />}
          {commitLine && <polyline points={commitLine} className="chart-line chart-commit" />}
        </svg>
        {history.length < 2 && <span className="chart-waiting">Collecting trend samples…</span>}
      </div>
      <div className="chart-footer"><span>Last {Math.max(1, history.length)} samples</span><span>Native sampling • no smoothing</span></div>
    </section>
  );
}

function CleanerPanel({ capability, actions, purging, onPurge }: { capability: ActionCapability; actions: ActionRecord[]; purging: boolean; onPurge: () => void }) {
  const lastAction = actions.at(-1) ?? null;
  const resultClass = lastAction?.nativeRequestSucceeded ? "is-success" : lastAction ? "is-failure" : "";
  const resultTitle = !lastAction
    ? "No purge recorded"
    : lastAction.nativeRequestSucceeded
      ? lastAction.observedReductionBytes !== null && lastAction.observedReductionBytes >= 1024 * 1024
        ? `${formatBytes(lastAction.observedReductionBytes)} standby reduction observed`
        : lastAction.observedReductionBytes === null ? "Windows accepted the request; measurement unavailable" : "Windows accepted the request; no material reduction observed"
      : lastAction.outcome === "elevation_cancelled"
        ? "Administrator approval was cancelled"
        : lastAction.lateCompletionPossible ? "Helper result pending or unknown" : "Standby purge did not complete";
  return (
    <section className={`panel cleaner-panel ${purging ? "is-running" : ""}`} aria-live="polite">
      <div className="cleaner-copy">
        <span className="eyebrow">Standby Cleaner</span>
        <h2>Release the Windows standby list on demand.</h2>
        <p>MemFlux captures real counters before and after the native request. Clearing cached data can cause later disk reads and does not guarantee better performance.</p>
        <div className={`cleaner-capability capability-${capability.status}`}>
          <span className="status-dot" />
          <span><strong>{capability.status === "ready" ? "Native action ready" : capability.status === "requires_elevation" ? "Administrator approval required" : capability.status === "unverified" ? "Windows build awaiting validation" : "Native action unavailable"}</strong><small>{capability.explanation}</small></span>
        </div>
        <button className="purge-button" disabled={!capability.supported || purging} onClick={onPurge}>
          <Icon name="pulse" size={21} />
          <span>{purging ? "PURGING STANDBY MEMORY…" : "PURGE STANDBY MEMORY"}<small>{capability.requiresElevation ? "Windows UAC will request approval" : "Uses the validated native memory-list operation"}</small></span>
        </button>
      </div>
      <div className={`cleaner-result ${resultClass}`}>
        <span className="eyebrow">Last action</span>
        <h3>{resultTitle}</h3>
        {lastAction ? <>
          <div className="result-grid">
            <span><small>Standby before</small><strong>{formatBytes(lastAction.before?.standbyBytes ?? null)}</strong></span>
            <span><small>Standby after</small><strong>{formatBytes(lastAction.after?.standbyBytes ?? null)}</strong></span>
            <span><small>Free + zero before</small><strong>{formatBytes(lastAction.before?.freeZeroBytes ?? null)}</strong></span>
            <span><small>Free + zero after</small><strong>{formatBytes(lastAction.after?.freeZeroBytes ?? null)}</strong></span>
          </div>
          <p>{lastAction.reason}</p>
          <div className="result-meta"><code>{lastAction.ntStatusHex ?? "no native status"}</code><span>{lastAction.durationMs} ms total</span><span>{formatTimestamp(lastAction.timestampMs)}</span></div>
          {actions.length > 1 && <div className="action-history"><strong>Recent actions</strong>{actions.slice(-4).reverse().map((record) => <div key={record.id}><span>{formatTimestamp(record.timestampMs)}</span><code>{record.ntStatusHex ?? "—"}</code><span>{formatBytes(record.observedReductionBytes)}</span></div>)}</div>}
        </> : <p>Manual actions are serialized and retained locally in a bounded history across restarts. Native acceptance and measured change are reported separately.</p>}
      </div>
    </section>
  );
}

function OverviewPage({ frame, history, capability, actions, purging, onPurge }: { frame: TelemetryFrame; history: TelemetryFrame[]; capability: ActionCapability; actions: ActionRecord[]; purging: boolean; onPurge: () => void }) {
  const { snapshot } = frame;
  return (
    <div className="page-stack">
      <PageTitle eyebrow="Command center" title="Standby memory, under your control." description="Real Windows memory telemetry around a genuine, measured standby-list purge." side={<div className="timestamp-card"><span>Last native sample</span><strong>{formatTimestamp(snapshot.timestampMs)}</strong><small>{snapshot.sampleIntervalMs / 1000}s cadence</small></div>} />
      {snapshot.collectionStatus !== "monitoring" && <div className="degraded-banner"><Icon name="info" /><span><strong>{snapshot.collectionStatus === "degraded" ? "Partial telemetry" : "Telemetry issue"}</strong>{snapshot.collectionDetail ?? "One or more sensors are unavailable."}</span></div>}
      <section className="metric-grid">
        <MetricCard label="Standby cache" metric={snapshot.standbyBytes} formatter={formatBytes} context="Reclaimable file and application cache" accent="#a978ff" />
        <MetricCard label="Free + zero" metric={snapshot.freeZeroBytes} formatter={formatBytes} context="Unassigned physical pages" accent="#77d9b4" />
        <MetricCard label="Available memory" metric={snapshot.availablePhysicalBytes} formatter={formatBytes} context="Includes reclaimable standby" accent="#78beff" />
        <MetricCard label="Physical load" metric={snapshot.physicalLoadPercent} formatter={(value) => formatPercent(value, 1)} context={`${formatBytes(snapshot.totalPhysicalBytes.value)} total physical memory`} accent="#d58fff" />
      </section>
      <CleanerPanel capability={capability} actions={actions} purging={purging} onPurge={onPurge} />
      <div className="panel-note" role="status">Automatic policy: {(frame.cleanerDecision ?? "disabled").replaceAll("_", " ")}. Unattended purge authorization is not yet available.</div>
      <section className="overview-hero">
        <MemoryDial snapshot={snapshot} />
        <GuardianPanel decision={frame.guardian} updated={snapshot.timestampMs} />
      </section>
      <div className="dashboard-grid"><MemoryComposition snapshot={snapshot} /><TrendGraph history={history} /></div>
    </div>
  );
}

function SensorRow({ label, metric, description }: { label: string; metric: Metric; description: string }) {
  return (
    <tr>
      <td><span className={`sensor-dot sensor-${metric.status}`} /><div><strong>{label}</strong><small>{description}</small></div></td>
      <td className="sensor-value">{formatMetric(metric)}</td>
      <td><code>{metric.source}</code></td>
      <td><span className={`availability-tag tag-${metric.status}`}>{statusLabel(metric.status)}</span>{metric.detail && <small className="sensor-detail">{metric.detail}</small>}</td>
    </tr>
  );
}

function ExplorerPage({ frame }: { frame: TelemetryFrame }) {
  const snapshot = frame.snapshot;
  const sensors: Array<{ label: string; metric: Metric; description: string }> = [
    { label: "Installed / usable physical", metric: snapshot.totalPhysicalBytes, description: "Physical memory visible to Windows" },
    { label: "Available physical", metric: snapshot.availablePhysicalBytes, description: "Includes standby, free, and zero pages" },
    { label: "Non-available physical", metric: snapshot.nonAvailablePhysicalBytes, description: "Derived total minus available" },
    { label: "Commit used", metric: snapshot.commitUsedBytes, description: "Committed virtual memory" },
    { label: "Commit limit", metric: snapshot.commitLimitBytes, description: "Current system commitment limit" },
    { label: "Standby cache", metric: snapshot.standbyBytes, description: "Reserve + normal + core standby classes" },
    { label: "Free + zero page list", metric: snapshot.freeZeroBytes, description: "Unassigned physical pages" },
    { label: "Modified page list", metric: snapshot.modifiedBytes, description: "Dirty pages awaiting writeback" },
    { label: "Page reads / sec", metric: snapshot.pageReadsPerSecond, description: "Hard read operations from backing store" },
    { label: "Processes", metric: snapshot.processCount, description: "System process count" },
    { label: "Threads", metric: snapshot.threadCount, description: "System thread count" }
  ];
  return (
    <div className="page-stack">
      <PageTitle eyebrow="Memory Explorer" title="Every number has provenance." description="Inspect collection status, source, cadence, and availability. Missing sensors stay missing—MemFlux never fills gaps with guesses." side={<div className="capability-chip"><span className={`sensor-dot sensor-${snapshot.collectionStatus === "monitoring" ? "available" : "error"}`} /><span><small>Collector state</small><strong>{snapshot.collectionStatus}</strong></span></div>} />
      <section className="panel sensor-panel">
        <div className="panel-heading"><div><span className="eyebrow">Sensor registry</span><h2>Current native measurements</h2></div><span className="sensor-caption">Sample #{snapshot.sequence} • {formatTimestamp(snapshot.timestampMs)}</span></div>
        <div className="sensor-table-wrap"><table className="sensor-table"><thead><tr><th>Measurement</th><th>Value</th><th>Source</th><th>Status</th></tr></thead><tbody>{sensors.map((sensor) => <SensorRow key={sensor.metric.id} {...sensor} />)}</tbody></table></div>
      </section>
      <section className="explainer-grid">
        <article className="panel text-panel"><span className="eyebrow">Correctness note</span><h2>Available is not the same as empty</h2><p>Windows can quickly reuse standby cache. A large standby value can coexist with healthy headroom; clearing it merely to increase a “free” counter is not evidence of better performance.</p></article>
        <article className="panel text-panel"><span className="eyebrow">OS pressure signal</span><h2>{snapshot.osMemoryState === "unavailable" ? "Signal unavailable" : `${snapshot.osMemoryState.charAt(0).toUpperCase()}${snapshot.osMemoryState.slice(1)} memory state`}</h2><p>The documented Windows resource notification is one input among several. Its neutral band is expected and never treated as an error.</p></article>
      </section>
    </div>
  );
}

function GuardianPage({ frame, paused, onPause, capability }: { frame: TelemetryFrame; paused: boolean; onPause: () => void; capability: ActionCapability }) {
  const decision = frame.guardian;
  return (
    <div className="page-stack">
      <PageTitle eyebrow="Adaptive Guardian" title="Assessment and cleaning policy stay distinct." description="Guardian explains pressure and safeguards. Manual standby clearing remains under direct user control and reports its own native outcome." side={<button className={`button ${paused ? "button-primary" : "button-quiet"}`} onClick={onPause}><Icon name={paused ? "play" : "pause"} />{paused ? "Resume Guardian" : "Pause Guardian"}</button>} />
      <GuardianPanel decision={decision} updated={frame.snapshot.timestampMs} />
      <div className="guardian-grid">
        <section className="panel evidence-panel"><div className="panel-heading"><div><span className="eyebrow">Decision trace</span><h2>Why Guardian chose {decision.outcome}</h2></div><code>{decision.reasonCode}</code></div><div className="evidence-list">{decision.evidence.length ? decision.evidence.map((item, index) => <div key={item}><span>{String(index + 1).padStart(2, "0")}</span><p>{item}</p></div>) : <div><span>—</span><p>No sensor evidence evaluated while paused.</p></div>}</div></section>
        <section className="panel safeguards-panel"><span className="eyebrow">Action boundary</span><h2>Native cleaner capability</h2><div className="capability-warning"><Icon name="shield" /><div><strong>{capability.status}</strong><p>{capability.explanation}</p></div></div><ul className="check-list"><li><span>✓</span> Unelevated monitoring process</li><li><span>✓</span> One fixed elevated operation</li><li><span>✓</span> Serialized purge requests</li><li><span>✓</span> Native status separated from measured change</li></ul></section>
      </div>
      <section className="panel policy-strip"><div><span className="policy-index">01</span><span><strong>Measure</strong><small>Pre-action counters</small></span></div><i /><div><span className="policy-index">02</span><span><strong>Authorize</strong><small>Narrow Windows UAC helper</small></span></div><i /><div><span className="policy-index">03</span><span><strong>Execute</strong><small>Native standby purge</small></span></div><i /><div><span className="policy-index">04</span><span><strong>Verify</strong><small>Post-action counters</small></span></div></section>
    </div>
  );
}

function Toggle({ checked, onChange, label, description }: { checked: boolean; onChange: (checked: boolean) => void; label: string; description: string }) {
  return (
    <label className="setting-row"><span><strong>{label}</strong><small>{description}</small></span><span className={`switch ${checked ? "is-on" : ""}`}><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /><span /></span></label>
  );
}

function SettingsPage({ envelope, saving, onSave }: { envelope: SettingsEnvelope; saving: boolean; onSave: (settings: AppSettings) => Promise<void> }) {
  const [draft, setDraft] = useState(envelope.settings);
  const [saved, setSaved] = useState(false);
  useEffect(() => setDraft(envelope.settings), [envelope.settings]);
  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => setDraft((current) => ({ ...current, [key]: value }));
  const changed = JSON.stringify(draft) !== JSON.stringify(envelope.settings);
  const submit = async () => { await onSave(draft); setSaved(true); window.setTimeout(() => setSaved(false), 1800); };
  return (
    <div className="page-stack">
      <PageTitle eyebrow="Settings" title="Quiet by default. Precise by choice." description="Tune monitoring behavior and presentation. Settings are validated by the native runtime and stored locally in a versioned file." side={<button className="button button-primary" disabled={!changed || saving} onClick={() => void submit()}>{saving ? "Saving…" : saved ? "Saved" : "Save changes"}</button>} />
      {envelope.warning && <div className="degraded-banner"><Icon name="info" /><span><strong>Safe defaults active</strong>{envelope.warning}</span></div>}
      <div className="settings-grid">
        <section className="panel settings-section"><span className="eyebrow">Cleaner timing</span><h2>Evaluation limits</h2><label className="select-row"><span><strong>Policy polling interval</strong><small>Evaluation follows available monitoring samples</small></span><select value={draft.cleanerPollIntervalMs} onChange={(event) => update("cleanerPollIntervalMs", Number(event.target.value))}><option value={1000}>1 second</option><option value={5000}>5 seconds</option><option value={10000}>10 seconds</option><option value={30000}>30 seconds</option><option value={60000}>60 seconds</option></select></label><label className="select-row"><span><strong>Minimum intervention interval</strong><small>Cooldown also applies</small></span><select value={draft.cleanerMinimumIntervalMs} onChange={(event) => update("cleanerMinimumIntervalMs", Number(event.target.value))}><option value={60000}>1 minute</option><option value={300000}>5 minutes</option><option value={600000}>10 minutes</option></select></label></section>
        <section className="panel settings-section cleaner-settings"><span className="eyebrow">Standby cleaner</span><h2>Automatic threshold policy</h2><Toggle checked={draft.automaticCleaningEnabled} onChange={(value) => update("automaticCleaningEnabled", value)} label="Enable automatic evaluation" description="Opt in to standby plus Free & Zero threshold evaluation. Background triggers never open surprise UAC prompts." /><Toggle checked={draft.cleanerPaused} onChange={(value) => update("cleanerPaused", value)} label="Pause automatic cleaner" description="Pause cleaner policy independently while telemetry continues." /><label className="select-row"><span><strong>Standby threshold</strong><small>Both threshold conditions must be true</small></span><select value={draft.standbyThresholdBytes} onChange={(event) => update("standbyThresholdBytes", Number(event.target.value))}><option value={536870912}>512 MiB</option><option value={1073741824}>1 GiB</option><option value={2147483648}>2 GiB</option><option value={4294967296}>4 GiB</option></select></label><label className="select-row"><span><strong>Free & Zero threshold</strong><small>Not available physical memory</small></span><select value={draft.freeZeroThresholdBytes} onChange={(event) => update("freeZeroThresholdBytes", Number(event.target.value))}><option value={536870912}>512 MiB</option><option value={1073741824}>1 GiB</option><option value={2147483648}>2 GiB</option><option value={4294967296}>4 GiB</option></select></label><label className="select-row"><span><strong>Cooldown</strong><small>Minimum time after a completed purge</small></span><select value={draft.cleanerCooldownMs} onChange={(event) => update("cleanerCooldownMs", Number(event.target.value))}><option value={300000}>5 minutes</option><option value={600000}>10 minutes</option><option value={1800000}>30 minutes</option><option value={3600000}>1 hour</option></select></label><label className="select-row"><span><strong>Maximum frequency</strong><small>Completed purges per rolling hour</small></span><select value={draft.maximumPurgesPerHour} onChange={(event) => update("maximumPurgesPerHour", Number(event.target.value))}><option value={1}>1 per hour</option><option value={2}>2 per hour</option><option value={4}>4 per hour</option></select></label><label className="select-row exclusions-row"><span><strong>Excluded foreground apps</strong><small>Comma-separated executable names</small></span><input value={draft.cleanerExcludedApplications.join(", ")} onChange={(event) => update("cleanerExcludedApplications", event.target.value.split(",").map((value) => value.trim()).filter(Boolean))} placeholder="obs64.exe, game.exe" /></label><p className="settings-caveat">Policy configuration and safeguards are active foundations. Unattended native execution remains disabled until the explicitly authorized session helper is complete; manual purge remains available.</p></section>
        <section className="panel settings-section"><span className="eyebrow">Guardian & lifecycle</span><h2>Background behavior</h2><Toggle checked={draft.guardianEnabled} onChange={(value) => update("guardianEnabled", value)} label="Adaptive Guardian" description="Interpret memory pressure separately from manual cleaning." /><Toggle checked={draft.closeToTray} onChange={(value) => update("closeToTray", value)} label="Close to system tray" description="Keep the collector running when the dashboard closes." /></section>
        <section className="panel settings-section"><span className="eyebrow">Sampling policy</span><h2>Monitoring cadence</h2><label className="select-row"><span><strong>Dashboard visible</strong><small>Faster feedback while you are looking</small></span><select value={draft.foregroundIntervalMs} onChange={(event) => update("foregroundIntervalMs", Number(event.target.value))}><option value={500}>0.5 seconds</option><option value={1000}>1 second</option><option value={2000}>2 seconds</option><option value={5000}>5 seconds</option></select></label><label className="select-row"><span><strong>Dashboard hidden</strong><small>Lower-overhead background cadence</small></span><select value={draft.backgroundIntervalMs} onChange={(event) => update("backgroundIntervalMs", Number(event.target.value))}><option value={2000}>2 seconds</option><option value={5000}>5 seconds</option><option value={10000}>10 seconds</option><option value={30000}>30 seconds</option></select></label></section>
        <section className="panel settings-section"><span className="eyebrow">Appearance</span><h2>Interface comfort</h2><Toggle checked={draft.reducedMotion} onChange={(value) => update("reducedMotion", value)} label="Reduced motion" description="Suppress non-essential transitions and ambient movement." /><Toggle checked={draft.compactDensity} onChange={(value) => update("compactDensity", value)} label="Compact density" description="Tighten cards and tables for information-dense windows." /></section>
        <section className="panel settings-section storage-section"><span className="eyebrow">Local-first privacy</span><h2>Settings storage</h2><div className="storage-path"><code>{envelope.path || "Resolving local application data path…"}</code></div><ul className="check-list"><li><span>✓</span> No account required</li><li><span>✓</span> No uploaded analytics</li><li><span>✓</span> Atomic local writes</li></ul></section>
      </div>
    </div>
  );
}

function UnavailablePanel({ title, detail }: { title: string; detail: string }) {
  return <section className="panel unavailable-panel"><Icon name="info" /><div><h2>{title}</h2><p>{detail}</p></div></section>;
}

export default function App() {
  const [page, setPage] = useState<PageId>("overview");
  const { frame, history, settingsEnvelope, runtime, error, saving, purging, actionHistory, setPaused, saveSettings, purgeStandby } = useMemFlux();
  const paused = runtime?.paused ?? false;
  const reducedMotion = settingsEnvelope?.settings.reducedMotion ?? false;
  const compactDensity = settingsEnvelope?.settings.compactDensity ?? false;

  useEffect(() => {
    document.documentElement.dataset.motion = reducedMotion ? "reduced" : "full";
    document.documentElement.dataset.density = compactDensity ? "compact" : "comfortable";
  }, [reducedMotion, compactDensity]);

  const content = useMemo(() => {
    if (!frame && page !== "settings") return <LoadingView />;
    if (page === "settings") return settingsEnvelope ? <SettingsPage envelope={settingsEnvelope} saving={saving} onSave={saveSettings} /> : <LoadingView />;
    if (!frame) return <LoadingView />;
    if (page === "explorer") return <ExplorerPage frame={frame} />;
    const capability = runtime?.actionCapability ?? { status: "unverified", compatibilityStatus: "unverified", supported: false, requiresElevation: false, privilegeState: "not_evaluated", mechanismAvailable: false, osBuild: null, mechanism: "Unavailable", explanation: "Capability detection has not completed." } satisfies ActionCapability;
    if (page === "guardian") return <GuardianPage frame={frame} paused={paused} onPause={() => void setPaused(!paused)} capability={capability} />;
    return <OverviewPage frame={frame} history={history} capability={capability} actions={actionHistory} purging={purging} onPurge={() => void purgeStandby()} />;
  }, [frame, page, history, settingsEnvelope, saving, saveSettings, paused, setPaused, runtime?.actionCapability, actionHistory, purging, purgeStandby]);

  return (
    <AppShell page={page} onPage={setPage} frame={frame} paused={paused} onPause={() => void setPaused(!paused)}>
      {error && <ErrorBanner message={error} />}
      {content}
    </AppShell>
  );
}

