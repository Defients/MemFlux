// Dev-only preview surface: renders the real dashboard against mocked native
// IPC so the interface can be reviewed in a plain browser (`npm run dev`, open
// /preview.html). Not referenced by index.html — excluded from production
// builds and the packaged app.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import "./styles.css";
import type {
  ActionRecord,
  AppSettings,
  AtlasReport,
  CleanerSessionStatus,
  CleanerStatusSummary,
  CoordinatorSnapshot,
  FpsCaptureStatus,
  GpuTelemetry,
  GuardianDecision,
  MemorySnapshot,
  Metric,
  PowerShiftOutcome,
  ProofReport,
  RewindEntry,
  RuntimeStatus,
  SensorReading,
  SettingsEnvelope,
  TelemetryFrame
} from "./types";

const now = () => Date.now();
let tick = 0;

const metric = (id: string, value: number | null, unit: string, source: string, status: Metric["status"] = "available", detail: string | null = null): Metric => ({
  id,
  value,
  unit,
  source,
  timestampMs: now(),
  sampleIntervalMs: 1000,
  status,
  detail
});

const gib = (value: number) => value * 1024 * 1024 * 1024;

const state = {
  paused: false,
  fpsActive: true,
  gamefocusActive: true,
  activeScheme: "High performance",
  sessionAuthorized: false,
  settings: {
    performanceHudEnabled: true,
    hudShowCpu: true,
    hudShowGpu: true,
    hudShowRam: true,
    hudShowFps: true,
    hudShowFrameTime: true,
    hudShowCleanerState: true,
    hudDetailLevel: "detailed",
    fpsCapturePermitted: true,
    schemaVersion: 5,
    guardianEnabled: true,
    closeToTray: true,
    foregroundIntervalMs: 1000,
    backgroundIntervalMs: 5000,
    reducedMotion: false,
    compactDensity: false,
    automaticCleaningEnabled: false,
    cleanerPaused: false,
    cleanerPollIntervalMs: 5000,
    standbyThresholdBytes: 2147483648,
    freeZeroThresholdBytes: 1073741824,
    cleanerCooldownMs: 600000,
    cleanerMinimumIntervalMs: 300000,
    maximumPurgesPerHour: 2,
    cleanerExcludedApplications: ["obs64.exe"],
    gamefocusEnabled: true,
    gamefocusAutoDetect: true,
    onboardingCompleted: false,
    miniHudPreset: "detailed",
    gameProfiles: [
      {
        id: "profile-demo-1",
        name: "Cyberpunk 2077",
        executable: "cyberpunk2077.exe",
        preset: "performance_gaming",
        powershiftMode: "performance",
        customPowerSchemeGuid: null,
        cleanerOverride: "enable_during_session",
        autoFpsCapture: true,
        approvedPowerIntervention: true,
        protectedApplications: ["obs64.exe", "discord.exe", "audiodg.exe"],
        sessionLogging: true
      },
      {
        id: "profile-demo-2",
        name: "Baldur's Gate 3",
        executable: "bg3.exe",
        preset: "balanced_gaming",
        powershiftMode: "balanced",
        customPowerSchemeGuid: null,
        cleanerOverride: "keep_global",
        autoFpsCapture: false,
        approvedPowerIntervention: true,
        protectedApplications: ["obs64.exe", "discord.exe", "audiodg.exe"],
        sessionLogging: true
      }
    ]
  } as AppSettings
};

const gpu = (): GpuTelemetry => ({
  utilization: metric(
    "gpu_utilization_percent",
    42 + 18 * Math.sin(tick / 6),
    "percent",
    "WDDM GPU Engine (NVIDIA GeForce RTX 4070: Busiest engine 3D)",
    "available",
    "NVIDIA GeForce RTX 4070 — Busiest engine: 3D"
  ),
  selectedAdapter: {
    luidKey: "0x00000000_0x00017a11",
    name: "NVIDIA GeForce RTX 4070",
    vendorId: 0x10de,
    deviceId: 0x2786,
    dedicatedVideoMemoryBytes: gib(12),
    sharedSystemMemoryBytes: gib(16),
    isHardware: true
  },
  availableAdapters: [
    {
      luidKey: "0x00000000_0x00017a11",
      name: "NVIDIA GeForce RTX 4070",
      vendorId: 0x10de,
      deviceId: 0x2786,
      dedicatedVideoMemoryBytes: gib(12),
      sharedSystemMemoryBytes: gib(16),
      isHardware: true
    }
  ],
  engineBreakdown: {
    busiestEngineType: "3D",
    threeDPercent: 42 + 18 * Math.sin(tick / 6),
    computePercent: 4,
    copyPercent: 3,
    videoDecodePercent: 1.2,
    videoEncodePercent: 2.4,
    otherPercent: 0
  }
});

const fpsCapture = (): FpsCaptureStatus => ({
  active: state.fpsActive,
  providerAvailable: true,
  providerName: "PresentMon",
  targetPid: state.fpsActive ? 8124 : null,
  targetExecutable: state.fpsActive ? "cyberpunk2077.exe" : null,
  targetCreationStamp: state.fpsActive ? "133000000000000" : null,
  captureSessionId: state.fpsActive ? "capture-demo-1" : null,
  fps: metric("measured_fps", state.fpsActive ? 138 + 14 * Math.sin(tick / 5) : null, "fps", "PresentMon ETW", state.fpsActive ? "available" : "unavailable"),
  frameTimeMs: metric("frame_time_ms", state.fpsActive ? 7.2 + 0.6 * Math.sin(tick / 4) : null, "ms", "PresentMon ETW MsBetweenPresents", state.fpsActive ? "available" : "unavailable"),
  frameTimeP99Ms: state.fpsActive ? 11.4 : null,
  recentStutters: state.fpsActive
    ? [
        {
          id: "stutter-1",
          atMs: now() - 64000,
          observedMs: now() - 64000,
          providerQpc: "8821345678",
          providerSeconds: 881.234,
          frameTimeMs: 24.6,
          baselineP95Ms: 9.8,
          ratio: 2.5,
          dropped: false,
          severity: "severe",
          note: "Frame-time outlier against rolling p95 baseline"
        },
        {
          id: "stutter-2",
          atMs: now() - 31000,
          observedMs: now() - 31000,
          providerQpc: "8845123456",
          providerSeconds: 883.512,
          frameTimeMs: 17.9,
          baselineP95Ms: 9.9,
          ratio: 1.8,
          dropped: false,
          severity: "mild",
          note: "Frame-time outlier against rolling p95 baseline"
        }
      ]
    : [],
  stutterSummary: state.fpsActive
    ? { eventsLastMinute: 2, worstRatioLastMinute: 2.5, baselineP95Ms: 9.8, baselineFrames: 412 }
    : null,
  statusDetail: state.fpsActive
    ? "Capturing cyberpunk2077.exe (PID 8124) via PresentMon ETW — non-injecting."
    : "Provider ready; no capture active."
});

const snapshot = (): MemorySnapshot => ({
  sequence: 900 + tick,
  timestampMs: now(),
  sampleIntervalMs: 1000,
  collectionStatus: "monitoring",
  collectionDetail: null,
  osMemoryState: "neutral",
  totalPhysicalBytes: metric("total_physical_bytes", gib(32), "bytes", "GlobalMemoryStatusEx"),
  availablePhysicalBytes: metric("available_physical_bytes", gib(13.6) + 0.6 * gib(1) * Math.sin(tick / 8), "bytes", "GlobalMemoryStatusEx"),
  nonAvailablePhysicalBytes: metric("non_available_physical_bytes", gib(18.4) - 0.6 * gib(1) * Math.sin(tick / 8), "bytes", "Derived (total − available)"),
  physicalLoadPercent: metric("physical_load_percent", 56 + 6 * Math.sin(tick / 8), "percent", "GlobalMemoryStatusEx dwMemoryLoad"),
  commitUsedBytes: metric("commit_used_bytes", gib(21.5) + 0.4 * gib(1) * Math.cos(tick / 9), "bytes", "Performance counter \\Memory\\Committed Bytes"),
  commitLimitBytes: metric("commit_limit_bytes", gib(36.7), "bytes", "GlobalMemoryStatusEx ullTotalPageFile"),
  commitPercent: metric("commit_percent", 58 + 3 * Math.cos(tick / 9), "percent", "Derived committed/limit"),
  standbyBytes: metric("standby_bytes", gib(6.2) + 0.35 * gib(1) * Math.sin(tick / 7), "bytes", "Memory Performance Information standby lists"),
  modifiedBytes: metric("modified_bytes", gib(0.31), "bytes", "Memory Performance Information modified list"),
  freeZeroBytes: metric("free_zero_bytes", gib(5.8) + 0.3 * gib(1) * Math.cos(tick / 6), "bytes", "Memory Performance Information free + zero lists"),
  pageReadsPerSecond: metric("page_reads_per_second", 12 + 8 * Math.sin(tick / 3), "reads_per_second", "PDH \\Memory\\Page Reads/sec"),
  processCount: metric("process_count", 214, "count", "NtQuerySystemInformation SystemProcessInformation"),
  threadCount: metric("thread_count", 3108, "count", "NtQuerySystemInformation SystemProcessInformation")
});

const guardian = (): GuardianDecision => ({
  outcome: "observe",
  pressure: "watch",
  reasonCode: "GUARDIAN_WATCH_STANDBY_ELEVATED",
  explanation:
    "Standby cache is holding 6.2 GB of file data while Free & Zero pages sit at 5.8 GB. Pressure is comfortable and a purge is optional — automatic policy remains within quota.",
  evidence: [
    "Standby cache 6.2 GB above the configured 2.0 GB threshold for 3 consecutive samples",
    "Free & Zero page list 5.8 GB remains above the 1.0 GB floor — no reclaim pressure",
    "Physical load 56% and commit 58% both sit inside the comfortable band",
    "Page read rate 12.0/s is below the elevated-pressure marker for this session"
  ],
  confidence: "moderate",
  sustainedSamples: 3,
  actionCapability: "requires_elevation"
});

const cleanerStatus = (): CleanerStatusSummary => ({
  state: "configured_unauthorized",
  label: "Configured — awaiting session helper",
  decision: "authorization_required",
  authorizationReady: false,
  sessionOverride: "keep_global",
  purgesLastHour: 1,
  maximumPurgesPerHour: 2,
  cooldownRemainingMs: 0,
  minimumIntervalRemainingMs: 0,
  pollIntervalMs: 5000,
  lastEvaluationMs: now(),
  detail: "Thresholds would trigger, but automatic execution needs an authorized elevated session helper."
});

const rewindEntries = (): RewindEntry[] => [
  {
    actionId: "rewind-demo-1",
    initiatingFeature: "powershift",
    sessionId: "session-demo-1",
    targetIdentity: "Windows power scheme",
    previousState: "381b4222-f694-41f0-9685-ff5bb260df2e",
    previousStateLabel: "Balanced",
    intendedNewState: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
    intendedNewStateLabel: "High performance",
    observedAppliedState: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
    timestampMs: now() - 2400000,
    restoredTimestampMs: null,
    authorization: "user_initiated",
    restorationMethod: "power_set_active_scheme",
    reversible: true,
    status: "applied_verified",
    errorDetail: null
  },
  {
    actionId: "rewind-demo-2",
    initiatingFeature: "powershift",
    sessionId: null,
    targetIdentity: "Windows power scheme",
    previousState: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
    previousStateLabel: "High performance",
    intendedNewState: "381b4222-f694-41f0-9685-ff5bb260df2e",
    intendedNewStateLabel: "Balanced",
    observedAppliedState: "381b4222-f694-41f0-9685-ff5bb260df2e",
    timestampMs: now() - 86400000,
    restoredTimestampMs: now() - 86000000,
    authorization: "user_initiated",
    restorationMethod: "power_set_active_scheme",
    reversible: true,
    status: "restored",
    errorDetail: null
  }
];

const coordinator = (): CoordinatorSnapshot => ({
  gamefocus: {
    enabled: true,
    lifecycle: state.gamefocusActive ? "active" : "idle",
    sessionId: state.gamefocusActive ? "session-demo-1" : null,
    activePid: state.gamefocusActive ? 8124 : null,
    activeExecutableName: state.gamefocusActive ? "cyberpunk2077.exe" : null,
    activeExecutablePath: state.gamefocusActive ? "C:\\Games\\Cyberpunk 2077\\bin\\x64\\cyberpunk2077.exe" : null,
    selectedProfile: state.gamefocusActive ? state.settings.gameProfiles?.[0] ?? null : null,
    candidate: state.gamefocusActive
      ? null
      : { pid: 4420, executableName: "bg3.exe", executablePath: "C:\\Games\\Baldurs Gate 3\\bin\\bg3.exe" },
    sessionStartedMs: state.gamefocusActive ? now() - 2400000 : null,
    sessionDurationMs: state.gamefocusActive ? 2400000 : 0,
    activePowerSchemeName: state.activeScheme,
    activePowerSchemeGuid: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
    powerInterventionActive: state.gamefocusActive,
    cleanerOverride: "enable_during_session",
    streamingDetected: true,
    activeProtectedWorkloads: ["obs64.exe", "discord.exe"],
    competingWorkloads: [
      { pid: 1204, executableName: "chrome.exe", cpuPercent: 7.4, workingSetBytes: gib(1.9), gpuPercent: 2.1, isProtected: false },
      { pid: 5510, executableName: "obs64.exe", cpuPercent: 4.2, workingSetBytes: gib(0.7), gpuPercent: 1.3, isProtected: true },
      { pid: 3311, executableName: "discord.exe", cpuPercent: 1.8, workingSetBytes: gib(0.4), gpuPercent: 0.4, isProtected: true }
    ],
    recommendations: [
      "obs64.exe and discord.exe detected — protected from any priority reduction.",
      "chrome.exe holds 1.9 GB working set across 3 background processes during the session."
    ],
    restorationState: "Power scheme Balanced → High performance recorded for restoration on session end.",
    lastCompletedSummary: null
  },
  powershift: {
    supported: true,
    activeScheme: { guid: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", name: state.activeScheme, active: true, category: "AC" },
    availableSchemes: [
      { guid: "381b4222-f694-41f0-9685-ff5bb260df2e", name: "Balanced", active: state.activeScheme === "Balanced", category: "AC" },
      { guid: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", name: "High performance", active: state.activeScheme === "High performance", category: "AC" },
      { guid: "a1841308-3541-4fab-bc81-f71556f20b4a", name: "Power saver", active: state.activeScheme === "Power saver", category: "DC" }
    ],
    powerSupply: { acStatus: "ac_connected", onBattery: false, batteryPercent: null, batterySaverActive: false, warning: null },
    memfluxOwnsActiveChange: state.gamefocusActive,
    activeRewindEntry: state.gamefocusActive ? rewindEntries()[0] ?? null : null,
    performanceSchemeAvailable: true,
    balancedSchemeAvailable: true,
    capabilityNote: "PowerEnumerate + PowerSetActiveScheme verified; every mutation is journaled for Optimization Rewind before execution."
  },
  rewindEntries: rewindEntries(),
  activeInterventionsCount: state.gamefocusActive ? 1 : 0,
  cleanerOverride: "enable_during_session",
  events: []
});

const reading = (id: string, value: number, unit: string, target: string, limitation: string): SensorReading => ({
  sensorId: id,
  category: "memory",
  value,
  unit,
  status: "available",
  source: "Native sampler",
  targetIdentity: target,
  deviceIdentity: null,
  eventTimestampMs: now(),
  observationTimestampMs: now(),
  samplingIntervalMs: 1000,
  uncertaintyMs: 120,
  capabilityLevel: "measured",
  limitation
});

const atlas = (): AtlasReport => ({
  timestampMs: now(),
  summary:
    "Memory headroom is adequate for the active session; the largest measured constraint is standby-cache residency competing with game working set growth, not a hard commit limit.",
  findings: [
    {
      id: "finding-demo-1",
      title: "Standby residency is the most plausible, weakest bottleneck",
      explanation:
        "Standby cache holds 6.2 GB while free pages remain sufficient. No measured process-level attribution proves the game is stalled on cache reclamation — this is a correlation, not a confirmed cause.",
      confidence: "weak",
      evidence: [
        reading("standby_bytes", gib(6.2), "bytes", "System", "List totals cannot attribute residency to a process."),
        reading("page_reads_per_second", 12, "reads/s", "System", "Coarse 1 s sampling cannot isolate per-load stalls.")
      ],
      missingEvidence: ["Per-process standby attribution", "Fault-time correlation with game loading"],
      contradictoryEvidence: ["Free & Zero list is above the reclaim floor"],
      recommendedTest: "Record a 30-second baseline during gameplay, purge standby manually, then mark the boundary and compare frame-time means."
    }
  ],
  sensors: [
    reading("cpu_utilization", 38, "%", "System", "GetSystemTimes delta; kernel includes idle."),
    reading("gpu_utilization", 44, "%", "NVIDIA GeForce RTX 4070", "Busiest engine only; engines never summed."),
    reading("standby_bytes", gib(6.2), "bytes", "System", "Includes all standby priority classes."),
    reading("commit_percent", 58, "%", "System", "Committed bytes over commit limit.")
  ],
  domains: [
    { domain: "Memory & commit", status: "sufficient coverage", detail: "Physical, commit, standby, free/zero and modified lists measured at 1 s cadence." },
    { domain: "CPU", status: "sufficient coverage", detail: "GetSystemTimes busy delta; per-process attribution during incidents." },
    { domain: "GPU", status: "partial coverage", detail: "Busiest WDDM engine summary; per-engine breakdown when counters report." },
    { domain: "Disk & paging", status: "partial coverage", detail: "Page reads/sec only; latency and queue depth are not measured." },
    { domain: "Thermals & power", status: "unavailable", detail: "No thermal collector is implemented." },
    { domain: "Streaming overhead", status: "partial coverage", detail: "Protected workloads observed; encoder load is not attributed." }
  ],
  incidents: [
    {
      id: "incident-demo-1:stutter-1",
      sessionId: "capture-demo-1",
      targetIdentity: "cyberpunk2077.exe (PID 8124)",
      provider: "PresentMon ETW",
      providerQpc: "8821345678",
      providerSeconds: 881.234,
      eventTimestampMs: now() - 64000,
      observationTimestampMs: now() - 64000,
      frameTimeMs: 24.6,
      severity: "severe",
      windowStartMs: now() - 67000,
      windowEndMs: now() - 61000,
      contextSamples: 6,
      contextState: "captured",
      finding: {
        id: "incident-finding-1",
        title: "Memory pressure coincided with the stutter window",
        explanation: "Available dipped 1.1 GB within the ±3 s context window while page reads rose — coincidence, not causation.",
        confidence: "weak",
        evidence: [reading("available_physical_bytes", gib(12.9), "bytes", "System", "Coarse 1 s sampling limits attribution.")],
        missingEvidence: ["Per-frame fault source", "Storage latency samples"],
        contradictoryEvidence: [],
        recommendedTest: "Repeat capture with a controlled workload boundary."
      }
    }
  ],
  retainedSamples: 90,
  evaluationMicros: 412
});

const frame = (): TelemetryFrame => ({
  atlas: atlas(),
  cpuUtilization: metric("cpu_utilization_percent", 32 + 16 * Math.sin(tick / 6), "percent", "GetSystemTimes (idle-aware delta)"),
  gpuTelemetry: gpu(),
  fpsCapture: fpsCapture(),
  snapshot: snapshot(),
  guardian: guardian(),
  cleanerDecision: "authorization_required",
  cleanerStatus: cleanerStatus(),
  coordinator: coordinator()
});

const capability = (): RuntimeStatus["actionCapability"] => ({
  status: "requires_elevation",
  compatibilityStatus: "confirmed_supported",
  supported: true,
  requiresElevation: true,
  privilegeState: "not_evaluated",
  mechanismAvailable: true,
  osBuild: 26100,
  osRevision: 1,
  osEdition: "Windows 11 Pro",
  architecture: "x64",
  mechanism: "NtSetSystemInformation(SystemMemoryListInformation) via one-shot elevated helper",
  explanation: "Build 26100 is inside the validated allowlist. Manual purge is available; UAC approval is requested each time unless a session helper is authorized.",
  compatibilityMatrix: [
    {
      windowsEdition: "Windows 11 Pro",
      build: 26100,
      revision: 1,
      architecture: "x64",
      securityConfiguration: "Default",
      privilegeContext: "Elevated helper",
      nativeMechanismAvailable: true,
      nativeRequestResult: "0x00000000",
      observedMemoryBehavior: "Standby reduced 6.1 GB → 0.4 GB",
      errorHandling: "NTSTATUS propagated",
      testDate: "2026-10-12",
      testMethod: "Manual elevated purge",
      status: "confirmed_supported"
    },
    {
      windowsEdition: "Windows 11 Home Insider",
      build: 26340,
      revision: 0,
      architecture: "x64",
      securityConfiguration: "Default",
      privilegeContext: "Elevated helper",
      nativeMechanismAvailable: true,
      nativeRequestResult: "0x00000000",
      observedMemoryBehavior: "Standby reduced 0.2 GB → 0.02 GB",
      errorHandling: "NTSTATUS propagated",
      testDate: "2026-10-18",
      testMethod: "Manual elevated purge",
      status: "confirmed_supported"
    }
  ]
});

const runtime = (): RuntimeStatus => ({
  paused: state.paused,
  cleanerPaused: false,
  cleanerDecision: "authorization_required",
  cleanerStatus: cleanerStatus(),
  cleanerSession: {
    authorized: state.sessionAuthorized,
    mode: state.sessionAuthorized ? "session_helper" : "none",
    helperPid: state.sessionAuthorized ? 9910 : null,
    sessionId: state.sessionAuthorized ? "session-helper-demo" : null,
    startedTimestampMs: state.sessionAuthorized ? now() - 600000 : null,
    expiresTimestampMs: null,
    detail: state.sessionAuthorized
      ? "Authenticated elevated session helper is active on this user session."
      : "No elevated session helper. Automatic execution stays disabled."
  },
  automaticExecutionReady: false,
  actionCapability: capability()
});

const actions: ActionRecord[] = [
  {
    id: "action-demo-2",
    timestampMs: now() - 2400000,
    origin: "manual",
    operation: "purge_standby_list",
    outcome: "accepted_observed",
    nativeRequestSucceeded: true,
    nativeLifecycle: "helper_completed",
    elevationUsed: true,
    lateCompletionPossible: false,
    ntStatusHex: "0x00000000",
    before: { standbyBytes: gib(6.4), freeZeroBytes: gib(5.1), availableBytes: gib(13.1) },
    after: { standbyBytes: gib(0.4), freeZeroBytes: gib(10.9), availableBytes: gib(13.4) },
    observedReductionBytes: gib(6.0),
    durationMs: 1180,
    nativeDurationMs: 640,
    reason: "Windows accepted the request; standby reduced measurably after settling."
  },
  {
    id: "action-demo-1",
    timestampMs: now() - 86400000,
    origin: "manual",
    operation: "purge_standby_list",
    outcome: "elevation_cancelled",
    nativeRequestSucceeded: false,
    nativeLifecycle: "uac_cancelled",
    elevationUsed: false,
    lateCompletionPossible: false,
    ntStatusHex: null,
    before: { standbyBytes: gib(4.9), freeZeroBytes: gib(6.2), availableBytes: gib(12.8) },
    after: null,
    observedReductionBytes: null,
    durationMs: 420,
    nativeDurationMs: 0,
    reason: "Administrator approval was cancelled at the UAC prompt."
  }
];

const proof = (): ProofReport => ({
  hypothesisId: null,
  active: false,
  phase: null,
  baselineSamples: 0,
  postSamples: 0,
  comparisons: [],
  verdict: "No proof session has run. Begin a baseline to compare a change you choose yourself.",
  caveat: "Descriptive deltas only — they do not establish a performance benefit."
});

const envelope = (): SettingsEnvelope => ({
  settings: state.settings,
  warning: null,
  path: "C:\\Users\\Preview\\AppData\\Roaming\\MemFlux\\settings.json"
});

mockIPC(
  (cmd, args) => {
    const payload = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "get_latest_frame":
        return frame();
      case "get_settings":
        return envelope();
      case "get_runtime_status":
        return runtime();
      case "get_action_history":
        return actions;
      case "get_coordinator_snapshot":
        return coordinator();
      case "set_monitoring_paused":
        state.paused = Boolean(payload.paused);
        void emit("monitoring://state", !state.paused);
        return state.paused;
      case "update_settings":
        state.settings = payload.settings as AppSettings;
        void emit("settings://changed", state.settings);
        return state.settings;
      case "purge_standby_memory": {
        const record: ActionRecord = {
          id: `action-demo-${actions.length + 1}`,
          timestampMs: now(),
          origin: "manual",
          operation: "purge_standby_list",
          outcome: "accepted_observed",
          nativeRequestSucceeded: true,
          nativeLifecycle: "helper_completed",
          elevationUsed: true,
          lateCompletionPossible: false,
          ntStatusHex: "0x00000000",
          before: { standbyBytes: gib(6.2), freeZeroBytes: gib(5.8), availableBytes: gib(13.6) },
          after: { standbyBytes: gib(0.35), freeZeroBytes: gib(11.5), availableBytes: gib(13.9) },
          observedReductionBytes: gib(5.85),
          durationMs: 1120,
          nativeDurationMs: 610,
          reason: "Windows accepted the request; standby reduced measurably after settling."
        };
        actions.push(record);
        void emit("action://result", record);
        return record;
      }
      case "authorize_cleaner_session": {
        state.sessionAuthorized = true;
        const status: CleanerSessionStatus = {
          authorized: true,
          mode: "session_helper",
          helperPid: 9910,
          sessionId: "session-helper-demo",
          startedTimestampMs: now(),
          expiresTimestampMs: null,
          detail: "Authenticated elevated session helper is active on this user session."
        };
        return status;
      }
      case "revoke_cleaner_session":
        state.sessionAuthorized = false;
        return { authorized: false, mode: "none", helperPid: null, sessionId: null, startedTimestampMs: null, expiresTimestampMs: null, detail: "Session helper revoked." } satisfies CleanerSessionStatus;
      case "apply_powershift_mode": {
        const mode = payload.mode as string;
        const name = mode === "performance" ? "High performance" : mode === "balanced" ? "Balanced" : "Power saver";
        const previous = state.activeScheme;
        state.activeScheme = name;
        const outcome: PowerShiftOutcome = {
          actionExecuted: true,
          systemStateVerified: true,
          workloadEffect: "unmeasured",
          previousScheme: { guid: "previous", name: previous, active: false, category: "AC" },
          appliedScheme: { guid: "applied", name, active: true, category: "AC" },
          rewindEntry: rewindEntries()[0] ?? null,
          warnings: [],
          detail: `PowerSetActiveScheme verified the ${name} scheme.`
        };
        return outcome;
      }
      case "restore_rewind_entry":
        state.activeScheme = "Balanced";
        return { ...rewindEntries()[0], status: "restored", restoredTimestampMs: now() };
      case "restore_all_rewind":
        state.activeScheme = "Balanced";
        return rewindEntries().map((entry) => ({ ...entry, status: "restored" as const, restoredTimestampMs: now() }));
      case "activate_gamefocus_session":
        state.gamefocusActive = true;
        return coordinator().gamefocus;
      case "deactivate_gamefocus_session":
        state.gamefocusActive = false;
        state.activeScheme = "Balanced";
        return coordinator().gamefocus;
      case "start_fps_capture":
        state.fpsActive = true;
        return fpsCapture();
      case "stop_fps_capture":
        state.fpsActive = false;
        return fpsCapture();
      case "begin_proof_session":
        return { ...proof(), active: true, phase: "baseline" };
      case "mark_proof_change":
        return { ...proof(), active: true, phase: "post_change", baselineSamples: 30 };
      case "proof_session_status":
        return proof();
      case "end_proof_session":
        return proof();
      case "disable_mini_hud":
        return null;
      default:
        return null;
    }
  },
  { shouldMockEvents: true }
);

const root = document.getElementById("root");
if (!root) throw new Error("MemFlux root element is missing.");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>
);

const pushFrame = () => {
  tick += 1;
  void emit("telemetry://frame", frame());
};

// Warm the trend history quickly, then settle into a realistic cadence.
for (let i = 0; i < 26; i += 1) {
  window.setTimeout(pushFrame, 120 * i);
}
window.setInterval(pushFrame, 1500);
