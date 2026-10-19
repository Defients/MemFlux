import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { useMemFlux } from "./hooks/useMemFlux";
import { native } from "./lib/native";
import { PerformanceHud } from "./components/PerformanceHud";
import { BottleneckAtlas } from "./components/BottleneckAtlas";
import {
  availablePercent,
  formatBytes,
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
  CleanerSessionStatus,
  CleanerStatusSummary,
  CoordinatorSnapshot,
  GamePreset,
  GameProfile,
  GuardianDecision,
  MemorySnapshot,
  Metric,
  PageId,
  PowerShiftMode,
  PressureLevel,
  ProofReport,
  RuntimeStatus,
  SettingsEnvelope,
  TelemetryFrame,
  TelemetryHistoryFrame
} from "./types";

// Rewind V2 lifecycle: entries carrying a live restoration obligation.
const REWIND_UNRESOLVED: ReadonlySet<string> = new Set([
  "prepared",
  "applied_verified",
  "outcome_uncertain",
  "restoration_pending",
  "recovery_required",
  // Legacy v1 names (only seen if the journal was written by V4.1 and read pre-migration)
  "active",
  "pending"
]);

const REWIND_TAG_CLASS: Record<string, string> = {
  prepared: "tag-warming_up",
  applied_verified: "tag-warming_up",
  outcome_uncertain: "tag-unavailable",
  restoration_pending: "tag-unavailable",
  recovery_required: "tag-error",
  restored: "tag-available",
  external_change_preserved: "tag-available",
  failed_before_mutation: "tag-unavailable",
  irreversible: "tag-unavailable",
  // Legacy aliases
  active: "tag-warming_up",
  pending: "tag-warming_up",
  conflict_skipped: "tag-available",
  failed: "tag-unavailable"
};

const NAVIGATION: Array<{ id: PageId; label: string; caption: string; icon: IconName }> = [
  { id: "overview", label: "Overview", caption: "System telemetry", icon: "overview" },
  { id: "memory", label: "Memory Control", caption: "Cleaner & Guardian", icon: "shield" },
  { id: "performance", label: "Performance Control", caption: "GameFocus & PowerShift", icon: "pulse" },
  { id: "intelligence", label: "Bottleneck Atlas", caption: "Evidence & experiments", icon: "explorer" },
  { id: "settings", label: "Settings", caption: "HUD, profiles & policy", icon: "settings" }
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

function formatDurationMs(ms: number): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const hrs = Math.floor(totalSec / 3600);
  const mins = Math.floor((totalSec % 3600) / 60);
  const secs = totalSec % 60;
  if (hrs > 0) return `${hrs}h ${mins}m ${secs}s`;
  if (mins > 0) return `${mins}m ${secs}s`;
  return `${secs}s`;
}

function buildPresetProfile(name: string, executable: string, preset: GamePreset): GameProfile {
  const id = `profile-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
  const exe = executable.trim().toLowerCase();
  if (preset === "performance_gaming") {
    return {
      id,
      name,
      executable: exe,
      preset,
      powershiftMode: "performance",
      customPowerSchemeGuid: null,
      cleanerOverride: "enable_during_session",
      autoFpsCapture: true,
      approvedPowerIntervention: true,
      protectedApplications: ["obs64.exe", "discord.exe", "audiodg.exe"],
      sessionLogging: true
    };
  }
  if (preset === "gaming_and_streaming") {
    return {
      id,
      name,
      executable: exe,
      preset,
      powershiftMode: "balanced",
      customPowerSchemeGuid: null,
      cleanerOverride: "pause_during_session",
      autoFpsCapture: false,
      approvedPowerIntervention: true,
      protectedApplications: ["obs64.exe", "obs32.exe", "discord.exe", "audiodg.exe", "voicemeeter.exe"],
      sessionLogging: true
    };
  }
  return {
    id,
    name,
    executable: exe,
    preset,
    powershiftMode: preset === "balanced_gaming" ? "balanced" : "current_system_mode",
    customPowerSchemeGuid: null,
    cleanerOverride: "keep_global",
    autoFpsCapture: false,
    approvedPowerIntervention: preset === "balanced_gaming",
    protectedApplications: ["obs64.exe", "discord.exe", "audiodg.exe"],
    sessionLogging: true
  };
}

function OnboardingBanner({
  capability,
  frame,
  onComplete
}: {
  capability: ActionCapability;
  frame: TelemetryFrame;
  onComplete: () => void;
}) {
  const gpuName = frame.gpuTelemetry?.selectedAdapter?.name ?? "Standard WDDM Adapter";
  const powerScheme = frame.coordinator?.powershift.activeScheme?.name ?? "System Default";
  return (
    <section className="panel onboarding-panel" aria-label="First-run system capability guide">
      <div className="panel-heading">
        <div>
          <span className="eyebrow">Welcome to MemFlux 1.0 · CosmoTech™</span>
          <h2>System capability scan & control guide</h2>
        </div>
        <button className="button button-primary" onClick={onComplete}>
          Got it — dismiss guide
        </button>
      </div>
      <div className="onboarding-grid">
        <div className="onboarding-step">
          <span className="policy-index">01</span>
          <div>
            <strong>Detected Windows & Hardware</strong>
            <p>
              Build {capability.osBuild ?? "unknown"} ({capability.osEdition ?? "Windows"}) • GPU: {gpuName} • Active Power Scheme: {powerScheme}. Ordinary monitoring runs without administrator rights.
            </p>
          </div>
        </div>
        <div className="onboarding-step">
          <span className="policy-index">02</span>
          <div>
            <strong>Standby Cleaner & Session Authorization</strong>
            <p>
              {capability.supported
                ? "This build is qualified for native standby purging. You can purge manually or authorize an elevated session helper so automatic threshold cleaning runs without surprise UAC prompts."
                : "This Windows build is currently outside the validated standby-purge allowlist. Monitoring remains active while native cache clearing is safely disabled."}
            </p>
          </div>
        </div>
        <div className="onboarding-step">
          <span className="policy-index">03</span>
          <div>
            <strong>GameFocus, PowerShift & Rewind</strong>
            <p>
              GameFocus detects confirmed games, protects streaming tools (OBS, Discord, audio), and applies reversible Windows power schemes tracked by the Optimization Rewind journal.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function AppShell({
  page,
  onPage,
  frame,
  paused,
  onPause,
  settings,
  children
}: {
  page: PageId;
  onPage: (page: PageId) => void;
  frame: TelemetryFrame | null;
  paused: boolean;
  onPause: () => void;
  settings?: AppSettings | undefined;
  children: ReactNode;
}) {
  const hudEnabled = settings?.performanceHudEnabled ?? true;
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

        {hudEnabled && <PerformanceHud frame={frame} paused={paused} settings={settings} />}
        <div className="sidebar-footer">
          <div className="engine-status">
            <span className={`engine-orbit ${paused ? "is-paused" : ""}`}><Icon name="pulse" size={17} /></span>
            <span><strong>{paused ? "Monitoring paused" : "Engine online"}</strong><small>{frame ? `Sample #${frame.snapshot.sequence}` : "Initializing sensors"}</small></span>
          </div>
          <div className="version-row"><span>Release Candidate</span><span>v1.0.0</span></div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="breadcrumbs"><span>CosmoTech™ Intelligence</span><b>/</b><strong>{NAVIGATION.find((item) => item.id === page)?.label}</strong></div>
          <div className="topbar-actions">
            {frame?.coordinator?.gamefocus.lifecycle === "active" && (
              <span className="status-pill status-watch is-compact">
                <span className="status-dot" />
                GameFocus: {frame.coordinator.gamefocus.selectedProfile?.name ?? frame.coordinator.gamefocus.activeExecutableName}
              </span>
            )}
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
  return <div className="error-banner" role="alert"><Icon name="info" /><div><strong>Runtime notice</strong><span>{message}</span></div></div>;
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

function TrendGraph({ history }: { history: TelemetryHistoryFrame[] }) {
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

function CleanerPanel({
  capability,
  cleanerStatus,
  cleanerSession,
  actions,
  purging,
  authorizingSession,
  onPurge,
  onAuthorizeSession,
  onRevokeSession
}: {
  capability: ActionCapability;
  cleanerStatus?: CleanerStatusSummary | undefined;
  cleanerSession?: CleanerSessionStatus | undefined;
  actions: ActionRecord[];
  purging: boolean;
  authorizingSession?: boolean | undefined;
  onPurge: () => void;
  onAuthorizeSession?: (() => void) | undefined;
  onRevokeSession?: (() => void) | undefined;
}) {
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
          <span>
            <strong>
              {capability.status === "ready"
                ? "Native action ready"
                : capability.status === "requires_elevation"
                  ? "Administrator approval required"
                  : capability.status === "unverified"
                    ? "Windows build awaiting validation"
                    : "Native action unavailable"}
            </strong>
            <small>{capability.explanation}</small>
          </span>
        </div>
        <button className="purge-button" disabled={!capability.supported || purging} onClick={onPurge}>
          <Icon name="pulse" size={21} />
          <span>
            {purging ? "PURGING STANDBY MEMORY…" : "PURGE STANDBY MEMORY"}
            <small>
              {cleanerSession?.authorized
                ? "Uses active authenticated elevated session helper"
                : capability.requiresElevation
                  ? "Windows UAC will request approval"
                  : "Uses the validated native memory-list operation"}
            </small>
          </span>
        </button>

        {onAuthorizeSession && onRevokeSession && (
          <div className="session-auth-box">
            <div>
              <strong>Automatic Cleaner Session: {cleanerStatus?.label ?? "Disabled"}</strong>
              <small>
                {cleanerSession?.detail ??
                  "Authorize an elevated session helper to allow threshold-based automatic purging without background UAC prompts."}
              </small>
              {cleanerStatus && (
                <small className="quota-meta">
                  Hourly quota: {cleanerStatus.purgesLastHour}/{cleanerStatus.maximumPurgesPerHour} purges • Poll: {cleanerStatus.pollIntervalMs / 1000}s
                  {cleanerStatus.cooldownRemainingMs > 0 ? ` • Cooldown: ${formatDurationMs(cleanerStatus.cooldownRemainingMs)}` : ""}
                </small>
              )}
            </div>
            <div className="session-auth-actions">
              {cleanerSession?.authorized ? (
                <button className="button button-quiet" onClick={onRevokeSession}>
                  Revoke session
                </button>
              ) : (
                <button
                  className="button button-primary"
                  disabled={!capability.supported || authorizingSession}
                  onClick={onAuthorizeSession}
                >
                  {authorizingSession ? "Authorizing…" : "Authorize session helper"}
                </button>
              )}
            </div>
          </div>
        )}
      </div>
      <div className={`cleaner-result ${resultClass}`}>
        <span className="eyebrow">Last action ({lastAction?.origin ?? "none"})</span>
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
          {actions.length > 1 && <div className="action-history"><strong>Recent actions</strong>{actions.slice(-4).reverse().map((record) => <div key={record.id}><span>{formatTimestamp(record.timestampMs)} ({record.origin})</span><code>{record.ntStatusHex ?? "—"}</code><span>{formatBytes(record.observedReductionBytes)}</span></div>)}</div>}
        </> : <p>Manual and authorized automatic actions are serialized and retained locally in a bounded history across restarts. Native acceptance and measured change are reported separately.</p>}
      </div>
    </section>
  );
}

function OverviewPage({
  frame,
  history,
  capability,
  showOnboarding,
  onDismissOnboarding,
  onNavigate
}: {
  frame: TelemetryFrame;
  history: TelemetryHistoryFrame[];
  capability: ActionCapability;
  showOnboarding: boolean;
  onDismissOnboarding: () => void;
  onNavigate: (page: PageId) => void;
}) {
  const { snapshot } = frame;
  const gf = frame.coordinator?.gamefocus;
  const ps = frame.coordinator?.powershift;
  return (
    <div className="page-stack">
      <PageTitle
        eyebrow="Command center"
        title="Standby memory, under your control."
        description="Real Windows memory telemetry, authorized automatic cleaning, and reversible GameFocus + PowerShift control."
        side={
          <div className="timestamp-card">
            <span>Last native sample</span>
            <strong>{formatTimestamp(snapshot.timestampMs)}</strong>
            <small>{snapshot.sampleIntervalMs / 1000}s cadence</small>
          </div>
        }
      />
      {showOnboarding && <OnboardingBanner capability={capability} frame={frame} onComplete={onDismissOnboarding} />}
      {snapshot.collectionStatus !== "monitoring" && (
        <div className="degraded-banner">
          <Icon name="info" />
          <span>
            <strong>{snapshot.collectionStatus === "degraded" ? "Partial telemetry" : "Telemetry issue"}</strong>
            {snapshot.collectionDetail ?? "One or more sensors are unavailable."}
          </span>
        </div>
      )}
      <section className="metric-grid">
        <MetricCard label="Standby cache" metric={snapshot.standbyBytes} formatter={formatBytes} context="Reclaimable file and application cache" accent="#a978ff" />
        <MetricCard label="Free + zero" metric={snapshot.freeZeroBytes} formatter={formatBytes} context="Unassigned physical pages" accent="#77d9b4" />
        <MetricCard label="Available memory" metric={snapshot.availablePhysicalBytes} formatter={formatBytes} context="Includes reclaimable standby" accent="#78beff" />
        <MetricCard label="Physical load" metric={snapshot.physicalLoadPercent} formatter={(value) => formatPercent(value, 1)} context={`${formatBytes(snapshot.totalPhysicalBytes.value)} total physical memory`} accent="#d58fff" />
      </section>
      {gf && (
        <section className="panel overview-session-strip">
          <div>
            <span className="eyebrow">GameFocus & PowerShift</span>
            <strong>
              {gf.lifecycle === "active" || gf.lifecycle === "temporarily_backgrounded"
                ? `Active Session: ${gf.selectedProfile?.name ?? gf.activeExecutableName} (${gf.lifecycle.replaceAll("_", " ")})`
                : gf.candidate
                  ? `Candidate Detected: ${gf.candidate.executableName} (PID ${gf.candidate.pid})`
                  : "GameFocus monitoring idle"}
            </strong>
            <small>
              Power Scheme: {ps?.activeScheme?.name ?? "Unknown"}{" "}
              {ps?.memfluxOwnsActiveChange ? "• Managed by MemFlux Rewind" : "• System / User owned"}
            </small>
          </div>
          <button className="button button-quiet" onClick={() => onNavigate("performance")}>
            Open Performance Control
          </button>
        </section>
      )}

      <section className="overview-hero">
        <MemoryDial snapshot={snapshot} />
      </section>
      <div className="dashboard-grid">
        <MemoryComposition snapshot={snapshot} />
        <TrendGraph history={history} />
      </div>
    </div>
  );
}

function MemoryControlPage({
  frame,
  paused,
  onPause,
  capability,
  runtime,
  actions,
  purging,
  authorizingSession,
  onPurge,
  onAuthorizeSession,
  onRevokeSession
}: {
  frame: TelemetryFrame;
  paused: boolean;
  onPause: () => void;
  capability: ActionCapability;
  runtime: RuntimeStatus | null;
  actions: ActionRecord[];
  purging: boolean;
  authorizingSession: boolean;
  onPurge: () => void;
  onAuthorizeSession: () => void;
  onRevokeSession: () => void;
}) {
  const decision = frame.guardian;
  const matrix = capability.compatibilityMatrix ?? [];
  return (
    <div className="page-stack">
      <PageTitle
        eyebrow="Memory Control & Adaptive Guardian"
        title="Assessment and cleaning policy stay distinct."
        description="Manual and authorized automatic standby purging operate with independent monotonic scheduling, while Guardian explains memory pressure."
        side={
          <button className={`button ${paused ? "button-primary" : "button-quiet"}`} onClick={onPause}>
            <Icon name={paused ? "play" : "pause"} />
            {paused ? "Resume Monitoring" : "Pause Monitoring"}
          </button>
        }
      />
      <CleanerPanel
        capability={capability}
        cleanerStatus={frame.cleanerStatus ?? runtime?.cleanerStatus}
        cleanerSession={runtime?.cleanerSession}
        actions={actions}
        purging={purging}
        authorizingSession={authorizingSession}
        onPurge={onPurge}
        onAuthorizeSession={onAuthorizeSession}
        onRevokeSession={onRevokeSession}
      />
      <GuardianPanel decision={decision} updated={frame.snapshot.timestampMs} />
      <div className="guardian-grid">
        <section className="panel evidence-panel">
          <div className="panel-heading">
            <div><span className="eyebrow">Decision trace</span><h2>Why Guardian chose {decision.outcome}</h2></div>
            <code>{decision.reasonCode}</code>
          </div>
          <div className="evidence-list">
            {decision.evidence.length ? (
              decision.evidence.map((item, index) => (
                <div key={item}><span>{String(index + 1).padStart(2, "0")}</span><p>{item}</p></div>
              ))
            ) : (
              <div><span>—</span><p>No sensor evidence evaluated while paused.</p></div>
            )}
          </div>
        </section>
        <section className="panel safeguards-panel">
          <span className="eyebrow">Privilege & compatibility boundary</span>
          <h2>Native cleaner capability</h2>
          <div className="capability-warning">
            <Icon name="shield" />
            <div>
              <strong>{capability.status} (Build {capability.osBuild ?? "—"})</strong>
              <p>{capability.explanation}</p>
            </div>
          </div>
          <ul className="check-list">
            <li><span>✓</span> Unelevated desktop monitoring process</li>
            <li><span>✓</span> Authenticated Named Pipe session helper with peer PID + image check</li>
            <li><span>✓</span> Independent monotonic scheduler & durable quota ledger</li>
            <li><span>✓</span> Native NTSTATUS separated from measured standby reduction</li>
          </ul>
        </section>
      </div>

      {matrix.length > 0 && (
        <section className="panel sensor-panel" style={{ marginTop: 14 }}>
          <div className="panel-heading">
            <div><span className="eyebrow">Windows qualification</span><h2>Compatibility matrix</h2></div>
            <span className="sensor-caption">Private memory-list operations require explicit build qualification</span>
          </div>
          <div className="sensor-table-wrap">
            <table className="sensor-table">
              <thead>
                <tr>
                  <th>Windows Edition / Build</th>
                  <th>Status</th>
                  <th>Native Result</th>
                  <th>Observed Behavior / Method</th>
                </tr>
              </thead>
              <tbody>
                {matrix.map((row) => (
                  <tr key={row.build}>
                    <td>
                      <div>
                        <strong>{row.windowsEdition}</strong>
                        <small>Build {row.build}{row.revision ? `.${row.revision}` : ""} ({row.architecture})</small>
                      </div>
                    </td>
                    <td>
                      <span className={`availability-tag ${row.status === "confirmed_supported" ? "tag-available" : "tag-unavailable"}`}>
                        {row.status.replaceAll("_", " ")}
                      </span>
                    </td>
                    <td><code>{row.nativeRequestResult}</code></td>
                    <td><small>{row.observedMemoryBehavior}</small></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}

function PerformanceControlPage({
  frame,
  settings,
  onSaveSettings,
  onApplyPowerShift,
  onRestoreRewindEntry,
  onRestoreAllRewind,
  onActivateGameFocus,
  onDeactivateGameFocus,
  onStartFpsCapture,
  onStopFpsCapture
}: {
  frame: TelemetryFrame;
  settings: AppSettings;
  onSaveSettings: (settings: AppSettings) => Promise<void>;
  onApplyPowerShift: (mode: PowerShiftMode, customGuid?: string | null) => Promise<unknown>;
  onRestoreRewindEntry: (actionId: string) => Promise<unknown>;
  onRestoreAllRewind: () => Promise<unknown>;
  onActivateGameFocus: (profile: GameProfile, pid: number | null, executablePath: string) => Promise<void>;
  onDeactivateGameFocus: () => Promise<void>;
  onStartFpsCapture: (pid: number, executableName: string) => Promise<void>;
  onStopFpsCapture: () => Promise<void>;
}) {
  const coord: CoordinatorSnapshot | undefined = frame.coordinator;
  const gf = coord?.gamefocus;
  const ps = coord?.powershift;
  const rewindEntries = coord?.rewindEntries ?? [];
  const profiles = settings.gameProfiles ?? [];

  const [newProfileName, setNewProfileName] = useState("");
  const [newProfileExe, setNewProfileExe] = useState("");
  const [newProfilePreset, setNewProfilePreset] = useState<GamePreset>("performance_gaming");
  const [customSchemeGuid, setCustomSchemeGuid] = useState<string>(ps?.activeScheme?.guid ?? "");

  const isSessionActive = gf?.lifecycle === "active" || gf?.lifecycle === "temporarily_backgrounded";

  const handleAddProfile = async () => {
    if (!newProfileName.trim() || !newProfileExe.trim()) return;
    const created = buildPresetProfile(newProfileName.trim(), newProfileExe.trim(), newProfilePreset);
    try {
      await onSaveSettings({
        ...settings,
        gameProfiles: [...profiles, created]
      });
    } catch {
      return; // failures surface through the shared error banner
    }
    setNewProfileName("");
    setNewProfileExe("");
  };

  const handleDeleteProfile = async (id: string) => {
    try {
      await onSaveSettings({
        ...settings,
        gameProfiles: profiles.filter((p) => p.id !== id)
      });
    } catch {
      // failures surface through the shared error banner
    }
  };

  const handleConfirmCandidate = async (preset: GamePreset) => {
    if (!gf?.candidate) return;
    const profile = buildPresetProfile(
      gf.candidate.executableName.replace(/\.exe$/i, ""),
      gf.candidate.executableName,
      preset
    );
    try {
      await onSaveSettings({
        ...settings,
        gameProfiles: [...profiles.filter((p) => p.executable !== profile.executable), profile]
      });
      await onActivateGameFocus(profile, gf.candidate.pid, gf.candidate.executablePath);
    } catch {
      // failures surface through the shared error banner
    }
  };

  return (
    <div className="page-stack">
      <PageTitle
        eyebrow="Performance Control"
        title="GameFocus & PowerShift."
        description="Detect gaming sessions, protect streaming workloads, and apply verified reversible Windows power configurations backed by Optimization Rewind."
        side={
          isSessionActive ? (
            <button className="button button-primary" onClick={() => void onDeactivateGameFocus().catch(() => undefined)}>
              Deactivate GameFocus and Restore
            </button>
          ) : undefined
        }
      />

      <div className="dashboard-grid">
        <section className="panel sensor-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">GameFocus Session</span>
              <h2>
                {isSessionActive
                  ? `${gf?.selectedProfile?.name ?? gf?.activeExecutableName} (${gf?.lifecycle.replaceAll("_", " ")})`
                  : "No active gaming session"}
              </h2>
            </div>
            <span className={`availability-tag ${isSessionActive ? "tag-available" : "tag-warming_up"}`}>
              {gf?.lifecycle.replaceAll("_", " ") ?? "idle"}
            </span>
          </div>

          <div className="result-grid" style={{ marginTop: 14 }}>
            <span>
              <small>Detected Executable</small>
              <strong>{gf?.activeExecutableName ?? gf?.candidate?.executableName ?? "None"}</strong>
            </span>
            <span>
              <small>Selected Profile</small>
              <strong>{gf?.selectedProfile?.name ?? "None"}</strong>
            </span>
            <span>
              <small>Current Power Scheme</small>
              <strong>{ps?.activeScheme?.name ?? "Unknown"}</strong>
            </span>
            <span>
              <small>Session Duration</small>
              <strong>{isSessionActive ? formatDurationMs(gf?.sessionDurationMs ?? 0) : "—"}</strong>
            </span>
            <span>
              <small>Cleaner Policy Override</small>
              <strong>{(gf?.cleanerOverride ?? "keep_global").replaceAll("_", " ")}</strong>
            </span>
            <span>
              <small>FPS Capture Status</small>
              <strong>{frame.fpsCapture?.active ? "Capturing" : "Inactive / Unavailable"}</strong>
            </span>
          </div>

          <p className="panel-note">
            <Icon name="shield" size={16} />
            <span>
              <strong>Restoration state:</strong> {gf?.restorationState ?? "No active changes."}
            </span>
          </p>

          {gf?.activeProtectedWorkloads && gf.activeProtectedWorkloads.length > 0 && (
            <div className="degraded-banner" style={{ marginTop: 12 }}>
              <Icon name="shield" />
              <span>
                <strong>Streaming / Protected Workloads Active:</strong> {gf.activeProtectedWorkloads.join(", ")} (preserved without priority reduction)
              </span>
            </div>
          )}

          {gf?.recommendations && gf.recommendations.length > 0 && (
            <div className="evidence-list" style={{ marginTop: 12 }}>
              {gf.recommendations.map((rec, i) => (
                <div key={rec}>
                  <span>{String(i + 1).padStart(2, "0")}</span>
                  <p>{rec}</p>
                </div>
              ))}
            </div>
          )}

          {!isSessionActive && gf?.candidate && (
            <div className="session-auth-box" style={{ marginTop: 14 }}>
              <div>
                <strong>Foreground Candidate: {gf.candidate.executableName} (PID {gf.candidate.pid})</strong>
                <small>{gf.candidate.executablePath}</small>
              </div>
              <div className="session-auth-actions">
                <button className="button button-primary" onClick={() => void handleConfirmCandidate("performance_gaming")}>
                  Confirm as Performance Game
                </button>
                <button className="button button-quiet" onClick={() => void handleConfirmCandidate("gaming_and_streaming")}>
                  Confirm for Gaming + Streaming
                </button>
              </div>
            </div>
          )}

          {isSessionActive && (
            <div className="session-auth-actions" style={{ marginTop: 14, display: "flex", gap: 10 }}>
              <button className="button button-primary" onClick={() => void onDeactivateGameFocus().catch(() => undefined)}>
                Deactivate GameFocus and Restore
              </button>
              {gf?.activePid && settings.fpsCapturePermitted && !frame.fpsCapture?.active && (
                <button
                  className="button button-quiet"
                  onClick={() => void onStartFpsCapture(gf.activePid!, gf.activeExecutableName ?? "game.exe").catch(() => undefined)}
                >
                  Start FPS Capture
                </button>
              )}
              {frame.fpsCapture?.active && (
                <button className="button button-quiet" onClick={() => void onStopFpsCapture().catch(() => undefined)}>
                  Stop FPS Capture
                </button>
              )}
            </div>
          )}
        </section>

        <section className="panel sensor-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">PowerShift</span>
              <h2>Windows Power Scheme Control</h2>
            </div>
            <span className={`availability-tag ${ps?.memfluxOwnsActiveChange ? "tag-available" : "tag-warming_up"}`}>
              {ps?.memfluxOwnsActiveChange ? "MemFlux Rewind Owned" : "System Owned"}
            </span>
          </div>

          <div className="result-grid" style={{ marginTop: 14 }}>
            <span>
              <small>Active Windows Scheme</small>
              <strong>{ps?.activeScheme?.name ?? "Unavailable"}</strong>
            </span>
            <span>
              <small>Power Supply / Battery</small>
              <strong>
                {ps?.powerSupply.acStatus.replaceAll("_", " ") ?? "unknown"}
                {ps?.powerSupply.batteryPercent !== null && ps?.powerSupply.batteryPercent !== undefined
                  ? ` (${ps.powerSupply.batteryPercent}%)`
                  : ""}
              </strong>
            </span>
            <span>
              <small>Previous Baseline Scheme</small>
              <strong>{ps?.activeRewindEntry?.previousStateLabel ?? "None recorded"}</strong>
            </span>
            <span>
              <small>Available Schemes</small>
              <strong>{ps?.availableSchemes.length ?? 0} enumerated</strong>
            </span>
          </div>

          {ps?.powerSupply.warning && (
            <div className="degraded-banner" style={{ marginTop: 12 }}>
              <Icon name="info" />
              <span>{ps.powerSupply.warning}</span>
            </div>
          )}

          <div className="powershift-controls" style={{ marginTop: 14, display: "flex", flexWrap: "wrap", gap: 8 }}>
            <button
              className="button button-quiet"
              disabled={!ps?.balancedSchemeAvailable}
              onClick={() => void onApplyPowerShift("balanced").catch(() => undefined)}
            >
              Apply Balanced Scheme
            </button>
            <button
              className="button button-primary"
              disabled={!ps?.performanceSchemeAvailable}
              onClick={() => void onApplyPowerShift("performance").catch(() => undefined)}
            >
              Apply Performance Scheme
            </button>
            {ps?.memfluxOwnsActiveChange && ps.activeRewindEntry && (
              <button
                className="button button-quiet"
                onClick={() => void onRestoreRewindEntry(ps.activeRewindEntry!.actionId).catch(() => undefined)}
              >
                Restore Previous ({ps.activeRewindEntry.previousStateLabel})
              </button>
            )}
          </div>

          {ps && ps.availableSchemes.length > 0 && (
            <div className="select-row" style={{ marginTop: 12 }}>
              <span>
                <strong>CustomEnumerated Scheme</strong>
                <small>Only real Windows schemes present on this machine are listed</small>
              </span>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <select
                  value={customSchemeGuid || ps.activeScheme?.guid || ""}
                  onChange={(e) => setCustomSchemeGuid(e.target.value)}
                >
                  {ps.availableSchemes.map((scheme) => (
                    <option key={scheme.guid} value={scheme.guid}>
                      {scheme.name} {scheme.active ? "(Active)" : ""}
                    </option>
                  ))}
                </select>
                <button
                  className="button button-quiet"
                  onClick={() => void onApplyPowerShift("custom", customSchemeGuid || ps.activeScheme?.guid).catch(() => undefined)}
                >
                  Activate
                </button>
              </div>
            </div>
          )}

          <p className="panel-note">
            <Icon name="info" size={16} />
            <span>{ps?.capabilityNote}</span>
          </p>
        </section>
      </div>

      <section className="panel sensor-panel" style={{ marginTop: 14 }}>
        <div className="panel-heading">
          <div>
            <span className="eyebrow">GameFocus Profiles</span>
            <h2>User-managed game library & presets</h2>
          </div>
          <span className="sensor-caption">Profiles apply temporary session overrides without overwriting global settings</span>
        </div>

        <div className="profile-creator" style={{ display: "flex", flexWrap: "wrap", gap: 10, marginTop: 14, alignItems: "center" }}>
          <input
            className="text-input"
            placeholder="Game title (e.g. Cyberpunk 2077)"
            value={newProfileName}
            onChange={(e) => setNewProfileName(e.target.value)}
          />
          <input
            className="text-input"
            placeholder="Executable (e.g. cyberpunk2077.exe)"
            value={newProfileExe}
            onChange={(e) => setNewProfileExe(e.target.value)}
          />
          <select value={newProfilePreset} onChange={(e) => setNewProfilePreset(e.target.value as GamePreset)}>
            <option value="balanced_gaming">Balanced Gaming</option>
            <option value="performance_gaming">Performance Gaming</option>
            <option value="gaming_and_streaming">Gaming + Streaming</option>
            <option value="custom">Custom (Observation Only)</option>
          </select>
          <button className="button button-primary" onClick={() => void handleAddProfile()}>
            Add Game Profile
          </button>
        </div>

        {profiles.length > 0 ? (
          <div className="sensor-table-wrap">
            <table className="sensor-table">
              <thead>
                <tr>
                  <th>Game / Executable</th>
                  <th>Preset</th>
                  <th>PowerShift Target</th>
                  <th>Cleaner Override / Actions</th>
                </tr>
              </thead>
              <tbody>
                {profiles.map((prof) => (
                  <tr key={prof.id}>
                    <td>
                      <div>
                        <strong>{prof.name}</strong>
                        <small><code>{prof.executable}</code></small>
                      </div>
                    </td>
                    <td>{prof.preset.replaceAll("_", " ")}</td>
                    <td>{prof.powershiftMode.replaceAll("_", " ")}</td>
                    <td>
                      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        <span>{prof.cleanerOverride.replaceAll("_", " ")}</span>
                        <button
                          className="button button-quiet"
                          onClick={() => void onActivateGameFocus(prof, null, prof.executable).catch(() => undefined)}
                        >
                          Activate
                        </button>
                        <button className="button button-quiet" onClick={() => void handleDeleteProfile(prof.id)}>
                          Remove
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="panel-note">No game profiles saved yet. Add an executable above or confirm a detected foreground candidate.</p>
        )}
      </section>

      <section className="panel sensor-panel" style={{ marginTop: 14 }}>
        <div className="panel-heading">
          <div>
            <span className="eyebrow">Optimization Rewind</span>
            <h2>Durable restoration journal (Compare-before-restore)</h2>
          </div>
          <button
            className="button button-quiet"
            disabled={!rewindEntries.some((e) => e.reversible && REWIND_UNRESOLVED.has(e.status))}
            onClick={() => void onRestoreAllRewind().catch(() => undefined)}
          >
            Restore All Pending Changes
          </button>
        </div>

        {rewindEntries.some((e) => e.status === "outcome_uncertain" || e.status === "recovery_required" || e.status === "restoration_pending") && (
          <p className="panel-note" style={{ color: "var(--status-warning)" }}>
            Some MemFlux changes have uncertain or unfinished restoration outcomes. They remain recoverable — MemFlux re-checks them on every restore pass and at startup.
          </p>
        )}

        {rewindEntries.length > 0 ? (
          <div className="sensor-table-wrap">
            <table className="sensor-table">
              <thead>
                <tr>
                  <th>Target & Feature</th>
                  <th>Previous → Applied State</th>
                  <th>Status</th>
                  <th>Action</th>
                </tr>
              </thead>
              <tbody>
                {rewindEntries.slice(-10).reverse().map((entry) => (
                  <tr key={entry.actionId}>
                    <td>
                      <div>
                        <strong>{entry.targetIdentity}</strong>
                        <small>{entry.initiatingFeature} • {formatTimestamp(entry.timestampMs)}</small>
                      </div>
                    </td>
                    <td>
                      <div>
                        <strong>{entry.previousStateLabel} → {entry.intendedNewStateLabel}</strong>
                        <small><code>{entry.observedAppliedState ?? entry.intendedNewState}</code></small>
                      </div>
                    </td>
                    <td>
                      <span className={`availability-tag ${REWIND_TAG_CLASS[entry.status] ?? "tag-unavailable"}`}>
                        {entry.status.replaceAll("_", " ")}
                      </span>
                      {entry.errorDetail && <small className="sensor-detail">{entry.errorDetail}</small>}
                    </td>
                    <td>
                      {entry.reversible && REWIND_UNRESOLVED.has(entry.status) ? (
                        <button className="button button-quiet" onClick={() => void onRestoreRewindEntry(entry.actionId).catch(() => undefined)}>
                          Restore
                        </button>
                      ) : (
                        <small>—</small>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="panel-note">No system mutations recorded yet. Every PowerShift change is journaled here before execution.</p>
        )}
      </section>
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

function IntelligencePage({
  frame,
  paused,
  onStartFpsCapture,
  onStopFpsCapture
}: {
  frame: TelemetryFrame;
  paused: boolean;
  onStartFpsCapture: (pid: number, executableName: string) => Promise<void>;
  onStopFpsCapture: () => Promise<void>;
}) {
  const snapshot = frame.snapshot;
  const gpu = frame.gpuTelemetry;
  const fps = frame.fpsCapture;
  const competing = frame.coordinator?.gamefocus.competingWorkloads ?? [];
  const [manualPid, setManualPid] = useState("");
  const [manualExe, setManualExe] = useState("game.exe");
  const [proof, setProof] = useState<ProofReport | null>(null);
  const [proofBusy, setProofBusy] = useState(false);
  const [proofError, setProofError] = useState<string | null>(null);

  const refreshProof = useCallback(() => {
    native
      .proofSessionStatus()
      .then(setProof)
      .catch(() => undefined);
  }, []);
  useEffect(() => {
    refreshProof();
  }, [refreshProof]);
  useEffect(() => {
    if (!proof?.active) return;
    const timer = window.setInterval(refreshProof, 1_500);
    return () => window.clearInterval(timer);
  }, [proof?.active, refreshProof]);
  const runProof = useCallback((action: () => Promise<ProofReport>) => {
    setProofBusy(true);
    setProofError(null);
    action()
      .then(setProof)
      .catch((reason: unknown) => setProofError(String(reason)))
      .finally(() => setProofBusy(false));
  }, []);

  const stutters = fps?.recentStutters ?? [];

  const sensors: Array<{ label: string; metric: Metric; description: string }> = [
    { label: "CPU utilization", metric: frame.cpuUtilization, description: "Overall CPU busy delta (idle included in kernel)" },
    ...(gpu ? [{ label: "GPU utilization (Busiest engine)", metric: gpu.utilization, description: gpu.selectedAdapter ? `${gpu.selectedAdapter.name} (${gpu.selectedAdapter.luidKey})` : "WDDM GPU Engine counter" }] : []),
    ...(fps ? [
      { label: "Measured FPS", metric: fps.fps, description: "Reciprocal mean MsBetweenPresents from PresentMon ETW" },
      { label: "Frame time (ms)", metric: fps.frameTimeMs, description: "Mean frame duration in milliseconds" }
    ] : []),
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
      <PageTitle
        eyebrow="Bottleneck Atlas"
        title="Find the constraint. Follow the evidence."
        description="Investigate measured conditions, inspect evidence gaps, and test a focused hypothesis."
        side={
          <div className="capability-chip">
            <span className={`sensor-dot sensor-${snapshot.collectionStatus === "monitoring" ? "available" : "error"}`} />
            <span><small>Collector state</small><strong>{snapshot.collectionStatus}</strong></span>
          </div>
        }
      />

      <BottleneckAtlas frame={frame} paused={paused} proofActive={proof?.active ?? false} onExperiment={(id) => runProof(() => native.beginProofSession(id))} />

      {gpu && gpu.availableAdapters.length > 0 && (
        <section className="panel sensor-panel" style={{ marginBottom: 14 }}>
          <div className="panel-heading">
            <div>
              <span className="eyebrow">WDDM GPU Attribution</span>
              <h2>{gpu.selectedAdapter?.name ?? "GPU Adapter"}</h2>
            </div>
            <span className="sensor-caption">
              Busiest engine summary (independent engines are never summed)
            </span>
          </div>
          <div className="result-grid" style={{ marginTop: 12 }}>
            <span>
              <small>Dedicated VRAM</small>
              <strong>{formatBytes(gpu.selectedAdapter?.dedicatedVideoMemoryBytes ?? null)}</strong>
            </span>
            <span>
              <small>3D / Graphics Engine</small>
              <strong>{formatPercent(gpu.engineBreakdown?.threeDPercent ?? null, 1)}</strong>
            </span>
            <span>
              <small>Compute Engine</small>
              <strong>{formatPercent(gpu.engineBreakdown?.computePercent ?? null, 1)}</strong>
            </span>
            <span>
              <small>Copy / Decode / Encode</small>
              <strong>
                {formatPercent(gpu.engineBreakdown?.copyPercent ?? null, 1)} /{" "}
                {formatPercent(gpu.engineBreakdown?.videoDecodePercent ?? null, 1)} /{" "}
                {formatPercent(gpu.engineBreakdown?.videoEncodePercent ?? null, 1)}
              </strong>
            </span>
          </div>
        </section>
      )}

      <section className="panel sensor-panel" style={{ marginBottom: 14 }}>
        <div className="panel-heading">
          <div>
            <span className="eyebrow">Non-injecting Frame Capture</span>
            <h2>PresentMon FPS & Frame-Time Session</h2>
          </div>
          <span className={`availability-tag ${fps?.active ? "tag-available" : "tag-unavailable"}`}>
            {fps?.active ? "Active Capture" : fps?.providerAvailable ? "Provider Ready" : "Provider Not Installed"}
          </span>
        </div>
        <p className="panel-note" style={{ marginTop: 10 }}>
          <Icon name="info" size={16} />
          <span>{fps?.statusDetail ?? "No FPS capture active."}</span>
        </p>
        <div style={{ display: "flex", gap: 10, marginTop: 12, alignItems: "center", flexWrap: "wrap" }}>
          <input
            className="text-input"
            placeholder="Target Process PID"
            value={manualPid}
            onChange={(e) => setManualPid(e.target.value)}
          />
          <input
            className="text-input"
            placeholder="Executable label"
            value={manualExe}
            onChange={(e) => setManualExe(e.target.value)}
          />
          <button
            className="button button-primary"
            disabled={!manualPid || !Number.isFinite(Number(manualPid))}
            onClick={() => void onStartFpsCapture(Number(manualPid), manualExe).catch(() => undefined)}
          >
            Start Capture
          </button>
          {fps?.active && (
            <button className="button button-quiet" onClick={() => void onStopFpsCapture().catch(() => undefined)}>
              Stop Capture
            </button>
          )}
        </div>
      </section>

      {fps?.active && (
        <section className="panel sensor-panel" style={{ marginBottom: 14 }}>
          <div className="panel-heading">
            <div>
              <span className="eyebrow">Stutter Detective</span>
              <h2>Frame-time anomalies against a rolling baseline</h2>
            </div>
            <span className="sensor-caption">
              {fps.stutterSummary
                ? `${fps.stutterSummary.eventsLastMinute} event(s)/min · baseline p95 ${fps.stutterSummary.baselineP95Ms?.toFixed(1) ?? "—"} ms from ${fps.stutterSummary.baselineFrames} frames`
                : "Warming up baseline"}
            </span>
          </div>
          {stutters.length === 0 ? (
            <p className="panel-note" style={{ marginTop: 10 }}>
              <Icon name="info" size={16} />
              <span>No frame-time anomalies detected yet. The detector needs ≥64 presented frames to establish a baseline.</span>
            </p>
          ) : (
            <div className="sensor-table-wrap">
              <table className="sensor-table">
                <thead>
                  <tr>
                    <th>When</th>
                    <th>Frame time</th>
                    <th>Baseline p95</th>
                    <th>Severity</th>
                    <th>Atlas investigation</th>
                  </tr>
                </thead>
                <tbody>
                  {stutters.slice().reverse().map((event) => {
                    const investigation = frame.atlas?.incidents.find((item) => item.sessionId === fps.captureSessionId && item.id.endsWith(`:${event.id}`));
                    return (
                      <tr key={event.id}>
                        <td><small>{event.atMs === null ? "Event time unavailable" : formatTimestamp(event.atMs)}</small></td>
                        <td className="sensor-value">{event.frameTimeMs.toFixed(1)} ms</td>
                        <td className="sensor-value">{event.baselineP95Ms.toFixed(1)} ms ({event.ratio.toFixed(1)}×)</td>
                        <td>
                          <span className={`availability-tag ${event.severity === "severe" || event.severity === "dropped_present" ? "tag-unavailable" : "tag-degraded"}`}>
                            {event.dropped ? "dropped" : event.severity}
                          </span>
                        </td>
                        <td>
                          <small>{investigation ? `${investigation.contextSamples} coarse samples · ${investigation.finding.title} · ${investigation.finding.confidence.replaceAll("_", " ")}` : "Awaiting native Atlas investigation"}</small>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
              <p className="settings-caveat" style={{ marginTop: 8 }}>
                Anomalies are frame-time outliers against a rolling p95 baseline. Atlas investigates a surrounding window of coarse measurements; missing event time suppresses correlation.
              </p>
            </div>
          )}
        </section>
      )}

      <section className="panel sensor-panel" style={{ marginBottom: 14 }}>
        <div className="panel-heading">
          <div>
            <span className="eyebrow">Performance Proof</span>
            <h2>Measure whether a change actually helped</h2>
          </div>
          <span className={`availability-tag ${proof?.active ? "tag-available" : "tag-unavailable"}`}>
            {proof?.active ? (proof.phase === "baseline" ? "Baseline Sampling" : "Post-change Sampling") : "Inactive"}
          </span>
        </div>
        <p className="panel-note" style={{ marginTop: 10 }}>
          <Icon name="info" size={16} />
          <span>Record comparable workload windows, choose a relevant intervention yourself, then mark the boundary. Each phase requires 30 seconds after a 5-second warm-up. Resource usage is context; these descriptive deltas do not establish a performance benefit.</span>
        </p>
        {proof?.hypothesisId && <p className="sensor-caption">Linked investigation: {proof.hypothesisId}</p>}
        <div style={{ display: "flex", gap: 10, marginTop: 12, alignItems: "center", flexWrap: "wrap" }}>
          {!proof?.active && (
            <button className="button button-primary" disabled={proofBusy} onClick={() => runProof(() => native.beginProofSession())}>
              Begin Baseline
            </button>
          )}
          {proof?.active && proof.phase === "baseline" && (
            <button className="button button-primary" disabled={proofBusy} onClick={() => runProof(() => native.markProofChange())}>
              Mark Change Applied
            </button>
          )}
          {proof?.active && (
            <button className="button button-quiet" disabled={proofBusy} onClick={() => runProof(() => native.endProofSession())}>
              Finish & Compare
            </button>
          )}
          {proof?.active && (
            <small className="sensor-caption">
              baseline {proof.baselineSamples} sample(s){proof.phase === "post_change" ? ` · post ${proof.postSamples} sample(s)` : ""}
            </small>
          )}
        </div>
        {proofError && <p role="alert">{proofError}</p>}
        {proof && (
          <>
            <p className="panel-note" style={{ marginTop: 12 }}>
              <Icon name="info" size={16} />
              <span>{proof.verdict}</span>
            </p>
            <div className="sensor-table-wrap">
              <table className="sensor-table">
                <thead>
                  <tr>
                    <th>Metric</th>
                    <th>Baseline (mean ± σ)</th>
                    <th>Post-change (mean ± σ)</th>
                    <th>Δ</th>
                    <th>Assessment</th>
                  </tr>
                </thead>
                <tbody>
                  {proof.comparisons.map((comparison) => (
                    <tr key={comparison.metric}>
                      <td><strong>{comparison.metric}</strong><small>{comparison.detail}</small></td>
                      <td className="sensor-value">{comparison.baselineMean.toFixed(1)} ± {comparison.baselineStddev.toFixed(1)}</td>
                      <td className="sensor-value">{comparison.postMean.toFixed(1)} ± {comparison.postStddev.toFixed(1)}</td>
                      <td className="sensor-value">{comparison.delta >= 0 ? "+" : ""}{comparison.delta.toFixed(1)} {comparison.unit}</td>
                      <td>
                        <span className={`availability-tag ${comparison.direction === "improved" ? "tag-available" : comparison.direction === "regressed" ? "tag-unavailable" : "tag-degraded"}`}>
                          {comparison.direction === "within_noise" ? "within noise" : comparison.direction}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {proof.caveat && <p className="settings-caveat" style={{ marginTop: 8 }}>{proof.caveat}</p>}
          </>
        )}
      </section>

      {competing.length > 0 && (
        <section className="panel sensor-panel" style={{ marginBottom: 14 }}>
          <div className="panel-heading">
            <div>
              <span className="eyebrow">Background Workload Intelligence</span>
              <h2>Processes active during GameFocus session</h2>
            </div>
            <span className="sensor-caption">Read-only contention analysis</span>
          </div>
          <div className="sensor-table-wrap">
            <table className="sensor-table">
              <thead>
                <tr>
                  <th>Process</th>
                  <th>CPU %</th>
                  <th>GPU Engine %</th>
                  <th>Working Set</th>
                </tr>
              </thead>
              <tbody>
                {competing.map((proc) => (
                  <tr key={proc.pid}>
                    <td>
                      <div>
                        <strong>{proc.executableName} (PID {proc.pid})</strong>
                        <small>{proc.isProtected ? "Protected streaming/audio workload" : "Background process"}</small>
                      </div>
                    </td>
                    <td className="sensor-value">{proc.cpuPercent.toFixed(1)}%</td>
                    <td className="sensor-value">{proc.gpuPercent.toFixed(1)}%</td>
                    <td className="sensor-value">{formatBytes(proc.workingSetBytes)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}

      <section className="panel sensor-panel">
        <div className="panel-heading"><div><span className="eyebrow">Sensor registry</span><h2>Current native measurements</h2></div><span className="sensor-caption">Sample #{snapshot.sequence} • {formatTimestamp(snapshot.timestampMs)}</span></div>
        <div className="sensor-table-wrap"><table className="sensor-table"><thead><tr><th>Measurement</th><th>Value</th><th>Source</th><th>Status</th></tr></thead><tbody>{sensors.map((sensor) => <SensorRow key={sensor.metric.id} {...sensor} />)}</tbody></table></div>
      </section>
    </div>
  );
}

function Toggle({ checked, onChange, label, description }: { checked: boolean; onChange: (checked: boolean) => void; label: string; description: string }) {
  return (
    <label className="setting-row"><span><strong>{label}</strong><small>{description}</small></span><span className={`switch ${checked ? "is-on" : ""}`}><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /><span /></span></label>
  );
}

export function SettingsPage({
  envelope,
  frame,
  saving,
  onSave
}: {
  envelope: SettingsEnvelope;
  frame: TelemetryFrame | null;
  saving: boolean;
  onSave: (settings: AppSettings) => Promise<void>;
}) {
  const [draft, setDraft] = useState(envelope.settings);
  const [saved, setSaved] = useState(false);
  const savedTimer = useRef<number | null>(null);
  useEffect(() => setDraft(envelope.settings), [envelope.settings]);
  useEffect(() => () => {
    if (savedTimer.current !== null) window.clearTimeout(savedTimer.current);
  }, []);
  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => setDraft((current) => ({ ...current, [key]: value }));
  const changed = JSON.stringify(draft) !== JSON.stringify(envelope.settings);
  const submit = async () => {
    try {
      await onSave(draft);
    } catch {
      return; // onSave surfaces the failure through the shared error banner
    }
    setSaved(true);
    if (savedTimer.current !== null) window.clearTimeout(savedTimer.current);
    savedTimer.current = window.setTimeout(() => {
      savedTimer.current = null;
      setSaved(false);
    }, 1800);
  };
  const adapters = frame?.gpuTelemetry?.availableAdapters ?? [];

  return (
    <div className="page-stack settings-page">
      <PageTitle eyebrow="Settings" title="Quiet by default. Precise by choice." description="Tune monitoring behavior, HUD metrics, GameFocus policies, and automatic standby cleaning. Validated and persisted atomically." />
      {envelope.warning && <div className="degraded-banner"><Icon name="info" /><span><strong>Safe defaults active</strong>{envelope.warning}</span></div>}
      <div className="settings-grid">
        <section className="panel settings-section">
          <span className="eyebrow">Live telemetry</span>
          <h2>Sidebar performance HUD</h2>
          <Toggle checked={draft.performanceHudEnabled} onChange={(value) => update("performanceHudEnabled", value)} label="Show performance HUD" description="Display live CPU, GPU, RAM, FPS, and cleaner status in the sidebar. Suspends when hidden." />
          <Toggle checked={draft.hudShowCpu ?? true} onChange={(value) => update("hudShowCpu", value)} label="Display CPU %" description="Overall CPU busy delta from GetSystemTimes." />
          <Toggle checked={draft.hudShowGpu ?? true} onChange={(value) => update("hudShowGpu", value)} label="Display GPU %" description="Busiest WDDM GPU engine utilization for the selected adapter." />
          <Toggle checked={draft.hudShowRam ?? true} onChange={(value) => update("hudShowRam", value)} label="Display RAM %" description="Physical memory load from GlobalMemoryStatusEx." />
          <Toggle checked={draft.hudShowFps ?? true} onChange={(value) => update("hudShowFps", value)} label="Display FPS & Frame Time" description="Show measured FPS and frame duration when capture is active." />
          <Toggle checked={draft.fpsCapturePermitted ?? false} onChange={(value) => update("fpsCapturePermitted", value)} label="Permit PresentMon FPS capture" description="Allow non-injecting ETW frame capture when requested." />
          <label className="select-row">
            <span><strong>Monitored GPU adapter</strong><small>Auto selects primary hardware GPU</small></span>
            <select value={draft.hudSelectedGpu ?? "auto"} onChange={(e) => update("hudSelectedGpu", e.target.value)}>
              <option value="auto">Automatic (Primary Hardware GPU)</option>
              {adapters.map((a) => (
                <option key={a.luidKey} value={a.luidKey}>{a.name} ({a.luidKey})</option>
              ))}
            </select>
          </label>
          <label className="select-row">
            <span><strong>HUD detail level</strong><small>Show adapter attribution in sidebar</small></span>
            <select value={draft.hudDetailLevel ?? "summary"} onChange={(e) => update("hudDetailLevel", e.target.value)}>
              <option value="summary">Summary</option>
              <option value="detailed">Detailed (Adapter + Engine)</option>
            </select>
          </label>
        </section>

        <section className="panel settings-section">
          <span className="eyebrow">GameFocus & PowerShift</span>
          <h2>Session & power safeguards</h2>
          <Toggle checked={draft.gamefocusEnabled ?? true} onChange={(value) => update("gamefocusEnabled", value)} label="Enable GameFocus session detection" description="Monitor foreground processes for configured game profiles and candidate games." />
          <Toggle checked={draft.gamefocusAutoDetect ?? true} onChange={(value) => update("gamefocusAutoDetect", value)} label="Auto-activate saved game profiles" description="Automatically start the matching GameFocus profile when a saved game enters focus." />
          <Toggle checked={draft.powershiftAllowOnBattery ?? false} onChange={(value) => update("powershiftAllowOnBattery", value)} label="Allow high-power schemes on battery" description="Conservative by default: block Performance power schemes when running on DC battery." />
          <label className="select-row">
            <span><strong>Cleaner policy polling interval</strong><small>Independent monotonic scheduler cadence</small></span>
            <select value={draft.cleanerPollIntervalMs} onChange={(event) => update("cleanerPollIntervalMs", Number(event.target.value))}>
              <option value={1000}>1 second</option>
              <option value={5000}>5 seconds</option>
              <option value={10000}>10 seconds</option>
              <option value={30000}>30 seconds</option>
              <option value={60000}>60 seconds</option>
            </select>
          </label>
          <label className="select-row">
            <span><strong>Minimum intervention interval</strong><small>Cooldown also applies</small></span>
            <select value={draft.cleanerMinimumIntervalMs} onChange={(event) => update("cleanerMinimumIntervalMs", Number(event.target.value))}>
              <option value={60000}>1 minute</option>
              <option value={300000}>5 minutes</option>
              <option value={600000}>10 minutes</option>
            </select>
          </label>
        </section>

        <section className="panel settings-section cleaner-settings">
          <span className="eyebrow">Standby cleaner</span>
          <h2>Automatic threshold policy</h2>
          <Toggle checked={draft.automaticCleaningEnabled} onChange={(value) => update("automaticCleaningEnabled", value)} label="Enable automatic evaluation" description="Opt in to standby plus Free & Zero threshold evaluation. Background triggers require an authorized session helper and never open surprise UAC prompts." />
          <Toggle checked={draft.cleanerPaused} onChange={(value) => update("cleanerPaused", value)} label="Pause automatic cleaner" description="Pause cleaner policy independently while telemetry continues." />
          <label className="select-row"><span><strong>Standby threshold</strong><small>Both threshold conditions must be true</small></span><select value={draft.standbyThresholdBytes} onChange={(event) => update("standbyThresholdBytes", Number(event.target.value))}><option value={536870912}>512 MiB</option><option value={1073741824}>1 GiB</option><option value={2147483648}>2 GiB</option><option value={4294967296}>4 GiB</option></select></label>
          <label className="select-row"><span><strong>Free & Zero threshold</strong><small>Not available physical memory</small></span><select value={draft.freeZeroThresholdBytes} onChange={(event) => update("freeZeroThresholdBytes", Number(event.target.value))}><option value={536870912}>512 MiB</option><option value={1073741824}>1 GiB</option><option value={2147483648}>2 GiB</option><option value={4294967296}>4 GiB</option></select></label>
          <label className="select-row"><span><strong>Cooldown</strong><small>Minimum time after a completed purge</small></span><select value={draft.cleanerCooldownMs} onChange={(event) => update("cleanerCooldownMs", Number(event.target.value))}><option value={300000}>5 minutes</option><option value={600000}>10 minutes</option><option value={1800000}>30 minutes</option><option value={3600000}>1 hour</option></select></label>
          <label className="select-row"><span><strong>Maximum frequency</strong><small>Completed purges per rolling hour</small></span><select value={draft.maximumPurgesPerHour} onChange={(event) => update("maximumPurgesPerHour", Number(event.target.value))}><option value={1}>1 per hour</option><option value={2}>2 per hour</option><option value={4}>4 per hour</option><option value={6}>6 per hour</option></select></label>
          <label className="select-row"><span><strong>Excluded foreground executables</strong><small>Comma-separated executable names or full paths</small></span><input value={draft.cleanerExcludedApplications.join(", ")} placeholder="game.exe, obs64.exe" onChange={(event) => update("cleanerExcludedApplications", event.target.value.split(",").map((item) => item.trim()).filter(Boolean))} /></label>
          <p className="settings-caveat">Unattended native execution runs only while an explicitly authorized elevated session helper is active. MemFlux never triggers background UAC prompts during gameplay.</p>
        </section>

        <section className="panel settings-section"><span className="eyebrow">Guardian & lifecycle</span><h2>Background behavior</h2><Toggle checked={draft.guardianEnabled} onChange={(value) => update("guardianEnabled", value)} label="Adaptive Guardian" description="Interpret memory pressure separately from manual cleaning." /><Toggle checked={draft.closeToTray} onChange={(value) => update("closeToTray", value)} label="Close to system tray" description="Keep the collector running when the dashboard closes." /></section>
        <section className="panel settings-section"><span className="eyebrow">Sampling policy</span><h2>Monitoring cadence</h2><label className="select-row"><span><strong>Dashboard visible</strong><small>Faster feedback while you are looking</small></span><select value={draft.foregroundIntervalMs} onChange={(event) => update("foregroundIntervalMs", Number(event.target.value))}><option value={500}>0.5 seconds</option><option value={1000}>1 second</option><option value={2000}>2 seconds</option><option value={5000}>5 seconds</option></select></label><label className="select-row"><span><strong>Dashboard hidden</strong><small>Lower-overhead background cadence</small></span><select value={draft.backgroundIntervalMs} onChange={(event) => update("backgroundIntervalMs", Number(event.target.value))}><option value={2000}>2 seconds</option><option value={5000}>5 seconds</option><option value={10000}>10 seconds</option><option value={30000}>30 seconds</option></select></label></section>
        <section className="panel settings-section"><span className="eyebrow">Tray & Mini HUD</span><h2>Low-footprint surfaces</h2>
          <Toggle checked={draft.ultraLightTrayMode ?? false} onChange={(value) => update("ultraLightTrayMode", value)} label="Ultra-Light Tray Mode" description="When the dashboard closes to tray, fully destroy the webview (frees the WebView2 processes). Monitoring, cleaner policy, and restoration keep running natively; Open dashboard recreates it." />
          <Toggle checked={draft.miniHudEnabled ?? false} onChange={(value) => update("miniHudEnabled", value)} label="Show Mini HUD" description="Independent floating monitor sharing the native sampler — useful while the dashboard stays hidden." />
          <Toggle checked={draft.miniHudAlwaysOnTop ?? true} onChange={(value) => update("miniHudAlwaysOnTop", value)} label="Mini HUD always on top" description="Keep the mini monitor above other windows." />
          <Toggle checked={draft.miniHudClickThrough ?? false} onChange={(value) => update("miniHudClickThrough", value)} label="Mini HUD click-through" description="Ignore pointer input so the HUD never steals clicks (reposition from the tray toggle instead)." />
          <label className="select-row"><span><strong>Mini HUD layout</strong><small>Compact metrics shown</small></span>
            <select value={draft.miniHudPreset ?? "compact"} onChange={(event) => update("miniHudPreset", event.target.value)}>
              <option value="minimal">Minimal (CPU + FPS)</option>
              <option value="compact">Compact (CPU · GPU · RAM · FPS)</option>
              <option value="detailed">Detailed (+ frame time, P99, cleaner)</option>
            </select>
          </label>
          <label className="select-row"><span><strong>Mini HUD opacity</strong><small>Visual transparency</small></span>
            <select value={draft.miniHudOpacityPercent ?? 92} onChange={(event) => update("miniHudOpacityPercent", Number(event.target.value))}>
              <option value={50}>50%</option>
              <option value={70}>70%</option>
              <option value={85}>85%</option>
              <option value={92}>92%</option>
              <option value={100}>100%</option>
            </select>
          </label>
        </section>
        <section className="panel settings-section"><span className="eyebrow">Appearance</span><h2>Interface comfort</h2><Toggle checked={draft.reducedMotion} onChange={(value) => update("reducedMotion", value)} label="Reduced motion" description="Suppress non-essential transitions and ambient movement." /><Toggle checked={draft.compactDensity} onChange={(value) => update("compactDensity", value)} label="Compact density" description="Tighten cards and tables for information-dense windows." /></section>
        <section className="panel settings-section storage-section"><span className="eyebrow">Local-first privacy</span><h2>Settings storage</h2><div className="storage-path"><code>{envelope.path || "Resolving local application data path…"}</code></div><ul className="check-list"><li><span>✓</span> No account required</li><li><span>✓</span> No uploaded analytics</li><li><span>✓</span> Atomic local writes</li></ul></section>
      </div>
      {(changed || saving || saved) && (
        <div className="save-floater" role="status" aria-live="polite">
          <span className="save-floater-label">{saving ? "Saving changes…" : changed ? "Unsaved changes" : "All changes saved"}</span>
          {changed && <button className="button button-quiet" disabled={saving} onClick={() => setDraft(envelope.settings)}>Discard</button>}
          <button className="button button-primary" disabled={!changed || saving} onClick={() => void submit()}>{saving ? "Saving…" : changed ? "Save changes" : "Saved"}</button>
        </div>
      )}
    </div>
  );
}

function UnavailablePanel({ title, detail }: { title: string; detail: string }) {
  return <section className="panel unavailable-panel"><Icon name="info" /><div><h2>{title}</h2><p>{detail}</p></div></section>;
}

const FALLBACK_CAPABILITY: ActionCapability = {
  status: "unverified",
  compatibilityStatus: "unverified",
  supported: false,
  requiresElevation: false,
  privilegeState: "not_evaluated",
  mechanismAvailable: false,
  osBuild: null,
  mechanism: "Unavailable",
  explanation: "Capability detection has not completed."
};

export default function App() {
  const [page, setPage] = useState<PageId>("overview");
  const {
    frame,
    history,
    settingsEnvelope,
    runtime,
    error,
    saving,
    purging,
    authorizingSession,
    actionHistory,
    setPaused,
    saveSettings,
    purgeStandby,
    authorizeCleanerSession,
    revokeCleanerSession,
    applyPowerShift,
    restoreRewindEntry,
    restoreAllRewind,
    activateGameFocus,
    deactivateGameFocus,
    startFpsCapture,
    stopFpsCapture
  } = useMemFlux();
  const paused = runtime?.paused ?? false;
  const reducedMotion = settingsEnvelope?.settings.reducedMotion ?? false;
  const compactDensity = settingsEnvelope?.settings.compactDensity ?? false;
  const showOnboarding = settingsEnvelope ? !settingsEnvelope.settings.onboardingCompleted : false;

  useEffect(() => {
    document.documentElement.dataset.motion = reducedMotion ? "reduced" : "full";
    document.documentElement.dataset.density = compactDensity ? "compact" : "comfortable";
  }, [reducedMotion, compactDensity]);

  const capability = runtime?.actionCapability ?? FALLBACK_CAPABILITY;

  const dismissOnboarding = useCallback(() => {
    if (!settingsEnvelope) return;
    void saveSettings({
      ...settingsEnvelope.settings,
      onboardingCompleted: true
    }).catch(() => undefined);
  }, [settingsEnvelope, saveSettings]);

  const content = useMemo(() => {
    if (!frame && page !== "settings") return <LoadingView />;
    if (page === "settings")
      return settingsEnvelope ? (
        <SettingsPage envelope={settingsEnvelope} frame={frame} saving={saving} onSave={saveSettings} />
      ) : (
        <LoadingView />
      );
    if (!frame) return <LoadingView />;
    if (page === "intelligence")
      return (
        <IntelligencePage
          frame={frame}
          paused={runtime?.paused ?? false}
          onStartFpsCapture={startFpsCapture}
          onStopFpsCapture={stopFpsCapture}
        />
      );
    if (page === "performance")
      return settingsEnvelope ? (
        <PerformanceControlPage
          frame={frame}
          settings={settingsEnvelope.settings}
          onSaveSettings={saveSettings}
          onApplyPowerShift={applyPowerShift}
          onRestoreRewindEntry={restoreRewindEntry}
          onRestoreAllRewind={restoreAllRewind}
          onActivateGameFocus={activateGameFocus}
          onDeactivateGameFocus={deactivateGameFocus}
          onStartFpsCapture={startFpsCapture}
          onStopFpsCapture={stopFpsCapture}
        />
      ) : (
        <LoadingView />
      );
    if (page === "memory")
      return (
        <MemoryControlPage
          frame={frame}
          paused={paused}
          onPause={() => void setPaused(!paused)}
          capability={capability}
          runtime={runtime}
          actions={actionHistory}
          purging={purging}
          authorizingSession={authorizingSession}
          onPurge={() => void purgeStandby().catch(() => undefined)}
          onAuthorizeSession={() => void authorizeCleanerSession().catch(() => undefined)}
          onRevokeSession={() => void revokeCleanerSession().catch(() => undefined)}
        />
      );
    return (
      <OverviewPage
        frame={frame}
        history={history}
        capability={capability}
        showOnboarding={showOnboarding}
        onDismissOnboarding={dismissOnboarding}
        onNavigate={setPage}
      />
    );
  }, [
    frame,
    page,
    history,
    settingsEnvelope,
    saving,
    saveSettings,
    paused,
    setPaused,
    capability,
    runtime,
    actionHistory,
    purging,
    authorizingSession,
    showOnboarding,
    dismissOnboarding,
    purgeStandby,
    authorizeCleanerSession,
    revokeCleanerSession,
    applyPowerShift,
    restoreRewindEntry,
    restoreAllRewind,
    activateGameFocus,
    deactivateGameFocus,
    startFpsCapture,
    stopFpsCapture
  ]);

  return (
    <AppShell
      page={page}
      onPage={setPage}
      frame={frame}
      paused={paused}
      onPause={() => void setPaused(!paused)}
      settings={settingsEnvelope?.settings}
    >
      {error && <ErrorBanner message={error} />}
      {content}
    </AppShell>
  );
}
