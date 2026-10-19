export type MetricStatus = "available" | "unavailable" | "error" | "warming_up";
export type CollectionStatus = "monitoring" | "degraded" | "error" | "paused" | "unsupported";
export type OsMemoryState = "low" | "high" | "neutral" | "unavailable";
export type GuardianOutcome = "observe" | "recommend";
export type PressureLevel = "comfortable" | "watch" | "elevated" | "critical" | "unknown" | "paused";

export interface Metric {
  id: string;
  value: number | null;
  unit: string;
  source: string;
  timestampMs: number;
  sampleIntervalMs: number;
  status: MetricStatus;
  detail: string | null;
}

export interface GpuAdapterInfo {
  luidKey: string;
  name: string;
  vendorId: number;
  deviceId: number;
  dedicatedVideoMemoryBytes: number;
  sharedSystemMemoryBytes: number;
  isHardware: boolean;
}

export interface GpuEngineBreakdown {
  busiestEngineType: string;
  threeDPercent: number;
  computePercent: number;
  copyPercent: number;
  videoDecodePercent: number;
  videoEncodePercent: number;
  otherPercent: number;
}

export interface GpuTelemetry {
  utilization: Metric;
  selectedAdapter: GpuAdapterInfo | null;
  availableAdapters: GpuAdapterInfo[];
  engineBreakdown: GpuEngineBreakdown | null;
}

export type StutterSeverity = "dropped_present" | "mild" | "moderate" | "severe";

export interface StutterEvent {
  id: string;
  atMs: number;
  frameTimeMs: number;
  baselineP95Ms: number;
  ratio: number;
  dropped: boolean;
  severity: StutterSeverity;
  note: string;
}

export interface StutterSummary {
  eventsLastMinute: number;
  worstRatioLastMinute: number | null;
  baselineP95Ms: number | null;
  baselineFrames: number;
}

export interface FpsCaptureStatus {
  active: boolean;
  providerAvailable: boolean;
  providerName: string;
  targetPid: number | null;
  targetExecutable: string | null;
  fps: Metric;
  frameTimeMs: Metric;
  frameTimeP99Ms: number | null;
  recentStutters: StutterEvent[];
  stutterSummary: StutterSummary | null;
  statusDetail: string;
}

export type ProofPhase = "baseline" | "post_change";

export interface MetricComparison {
  metric: string;
  unit: string;
  baselineMean: number;
  baselineStddev: number;
  postMean: number;
  postStddev: number;
  delta: number;
  direction: "improved" | "regressed" | "within_noise" | string;
  confidence: "measurable" | "inconclusive" | string;
  detail: string;
}

export interface ProofReport {
  active: boolean;
  phase: ProofPhase | null;
  baselineSamples: number;
  postSamples: number;
  comparisons: MetricComparison[];
  verdict: string;
  caveat: string;
}

export interface MemorySnapshot {
  sequence: number;
  timestampMs: number;
  sampleIntervalMs: number;
  collectionStatus: CollectionStatus;
  collectionDetail: string | null;
  osMemoryState: OsMemoryState;
  totalPhysicalBytes: Metric;
  availablePhysicalBytes: Metric;
  nonAvailablePhysicalBytes: Metric;
  physicalLoadPercent: Metric;
  commitUsedBytes: Metric;
  commitLimitBytes: Metric;
  commitPercent: Metric;
  standbyBytes: Metric;
  modifiedBytes: Metric;
  freeZeroBytes: Metric;
  pageReadsPerSecond: Metric;
  processCount: Metric;
  threadCount: Metric;
}

export interface GuardianDecision {
  outcome: GuardianOutcome;
  pressure: PressureLevel;
  reasonCode: string;
  explanation: string;
  evidence: string[];
  confidence: string;
  sustainedSamples: number;
  actionCapability: string;
}

export type SessionCleanerOverride = "keep_global" | "enable_during_session" | "pause_during_session";

export interface CleanerStatusSummary {
  state: string;
  label: string;
  decision: string;
  authorizationReady: boolean;
  sessionOverride: SessionCleanerOverride;
  purgesLastHour: number;
  maximumPurgesPerHour: number;
  cooldownRemainingMs: number;
  minimumIntervalRemainingMs: number;
  pollIntervalMs: number;
  lastEvaluationMs: number | null;
  detail: string;
}

export interface CleanerSessionStatus {
  authorized: boolean;
  mode: string;
  helperPid: number | null;
  sessionId: string | null;
  startedTimestampMs: number | null;
  expiresTimestampMs: number | null;
  detail: string;
}

export type PowerShiftMode = "current_system_mode" | "balanced" | "performance" | "custom";
export type GamePreset = "balanced_gaming" | "performance_gaming" | "gaming_and_streaming" | "custom";
export type GameSessionLifecycle =
  | "idle"
  | "candidate_detected"
  | "confirmed_game"
  | "profile_pending"
  | "active"
  | "temporarily_backgrounded"
  | "restoring"
  | "completed"
  | "degraded";

export interface GameProfile {
  id: string;
  name: string;
  executable: string;
  preset: GamePreset;
  powershiftMode: PowerShiftMode;
  customPowerSchemeGuid: string | null;
  cleanerOverride: SessionCleanerOverride;
  autoFpsCapture: boolean;
  approvedPowerIntervention: boolean;
  protectedApplications: string[];
  sessionLogging: boolean;
}

export interface CompetingProcessInfo {
  pid: number;
  executableName: string;
  cpuPercent: number;
  workingSetBytes: number;
  gpuPercent: number;
  isProtected: boolean;
}

export interface CandidateApplication {
  pid: number;
  executableName: string;
  executablePath: string;
}

export interface GameFocusStatus {
  enabled: boolean;
  lifecycle: GameSessionLifecycle;
  sessionId: string | null;
  activePid: number | null;
  activeExecutableName: string | null;
  activeExecutablePath: string | null;
  selectedProfile: GameProfile | null;
  candidate: CandidateApplication | null;
  sessionStartedMs: number | null;
  sessionDurationMs: number;
  activePowerSchemeName: string | null;
  activePowerSchemeGuid: string | null;
  powerInterventionActive: boolean;
  cleanerOverride: SessionCleanerOverride;
  streamingDetected: boolean;
  activeProtectedWorkloads: string[];
  competingWorkloads: CompetingProcessInfo[];
  recommendations: string[];
  restorationState: string;
  lastCompletedSummary: string | null;
}

export interface PowerSchemeInfo {
  guid: string;
  name: string;
  active: boolean;
  category: string;
}

export interface PowerSupplyStatus {
  acStatus: string;
  onBattery: boolean;
  batteryPercent: number | null;
  batterySaverActive: boolean;
  warning: string | null;
}

export type RewindStatus =
  | "prepared"
  | "applied_verified"
  | "outcome_uncertain"
  | "restoration_pending"
  | "restored"
  | "external_change_preserved"
  | "recovery_required"
  | "failed_before_mutation"
  | "irreversible"
  // Legacy v1 journal states still readable from disk
  | "pending"
  | "active"
  | "conflict_skipped"
  | "failed";

export type ShutdownPhase =
  | "running"
  | "shutdown_requested"
  | "draining_actions"
  | "stopping_capture"
  | "restoring_system_state"
  | "stopping_privileged_helper"
  | "flushing_persistence"
  | "shutdown_complete"
  | "shutdown_degraded";

export interface RewindEntry {
  actionId: string;
  initiatingFeature: string;
  sessionId: string | null;
  targetIdentity: string;
  previousState: string;
  previousStateLabel: string;
  intendedNewState: string;
  intendedNewStateLabel: string;
  observedAppliedState: string | null;
  timestampMs: number;
  restoredTimestampMs: number | null;
  authorization: string;
  restorationMethod: string;
  reversible: boolean;
  status: RewindStatus;
  errorDetail: string | null;
}

export interface PowerShiftStatus {
  supported: boolean;
  activeScheme: PowerSchemeInfo | null;
  availableSchemes: PowerSchemeInfo[];
  powerSupply: PowerSupplyStatus;
  memfluxOwnsActiveChange: boolean;
  activeRewindEntry: RewindEntry | null;
  performanceSchemeAvailable: boolean;
  balancedSchemeAvailable: boolean;
  capabilityNote: string;
}

export interface PowerShiftOutcome {
  actionExecuted: boolean;
  systemStateVerified: boolean;
  workloadEffect: string;
  previousScheme: PowerSchemeInfo | null;
  appliedScheme: PowerSchemeInfo | null;
  rewindEntry: RewindEntry | null;
  warnings: string[];
  detail: string;
}

export interface CoordinatorEvent {
  id: string;
  timestampMs: number;
  subsystem: string;
  eventType: string;
  summary: string;
  detail: string;
}

export interface CoordinatorSnapshot {
  gamefocus: GameFocusStatus;
  powershift: PowerShiftStatus;
  rewindEntries: RewindEntry[];
  activeInterventionsCount: number;
  cleanerOverride: SessionCleanerOverride;
  events: CoordinatorEvent[];
}

export interface TelemetryFrame {
  cpuUtilization: Metric;
  gpuTelemetry?: GpuTelemetry;
  fpsCapture?: FpsCaptureStatus;
  snapshot: MemorySnapshot;
  guardian: GuardianDecision;
  cleanerDecision: string;
  cleanerStatus?: CleanerStatusSummary;
  coordinator?: CoordinatorSnapshot;
}

export interface AppSettings {
  performanceHudEnabled: boolean;
  hudShowCpu?: boolean;
  hudShowGpu?: boolean;
  hudShowRam?: boolean;
  hudShowFps?: boolean;
  hudShowFrameTime?: boolean;
  hudShowCleanerState?: boolean;
  hudSelectedGpu?: string;
  hudCompactMode?: boolean;
  hudDetailLevel?: string;
  hudRefreshIntervalMs?: number;
  fpsCapturePermitted?: boolean;
  schemaVersion: number;
  guardianEnabled: boolean;
  closeToTray: boolean;
  foregroundIntervalMs: number;
  backgroundIntervalMs: number;
  reducedMotion: boolean;
  compactDensity: boolean;
  automaticCleaningEnabled: boolean;
  cleanerPaused: boolean;
  cleanerPollIntervalMs: number;
  standbyThresholdBytes: number;
  freeZeroThresholdBytes: number;
  cleanerCooldownMs: number;
  cleanerMinimumIntervalMs: number;
  maximumPurgesPerHour: number;
  cleanerExcludedApplications: string[];
  gamefocusEnabled?: boolean;
  gamefocusAutoDetect?: boolean;
  powershiftAllowOnBattery?: boolean;
  gameProfiles?: GameProfile[];
  onboardingCompleted?: boolean;
  /** V4.2: destroy the dashboard webview when closed to tray (recreates on open). */
  ultraLightTrayMode?: boolean;
  miniHudEnabled?: boolean;
  miniHudAlwaysOnTop?: boolean;
  miniHudClickThrough?: boolean;
  miniHudOpacityPercent?: number;
  miniHudX?: number | null;
  miniHudY?: number | null;
  /** "minimal" | "compact" | "detailed" */
  miniHudPreset?: string;
}

export interface SettingsEnvelope {
  settings: AppSettings;
  warning: string | null;
  path: string;
}

export interface CompatibilityEntry {
  windowsEdition: string;
  build: number;
  revision: number | null;
  architecture: string;
  securityConfiguration: string;
  privilegeContext: string;
  nativeMechanismAvailable: boolean;
  nativeRequestResult: string;
  observedMemoryBehavior: string;
  errorHandling: string;
  testDate: string;
  testMethod: string;
  status: string;
}

export interface ActionCapability {
  status: "ready" | "requires_elevation" | "confirmed_unsupported" | "unverified" | "mechanism_unavailable";
  compatibilityStatus: "confirmed_supported" | "confirmed_unsupported" | "unverified" | "mechanism_unavailable";
  supported: boolean;
  requiresElevation: boolean;
  privilegeState: string;
  mechanismAvailable: boolean;
  osBuild: number | null;
  osRevision?: number | null;
  osEdition?: string | null;
  architecture?: string;
  mechanism: string;
  explanation: string;
  compatibilityMatrix?: CompatibilityEntry[];
}

export interface RuntimeStatus {
  paused: boolean;
  cleanerPaused: boolean;
  cleanerDecision: string;
  cleanerStatus?: CleanerStatusSummary;
  cleanerSession?: CleanerSessionStatus;
  automaticExecutionReady: boolean;
  actionCapability: ActionCapability;
  shuttingDown?: boolean;
  shutdownPhase?: ShutdownPhase;
}

export interface ActionMeasurement {
  standbyBytes: number | null;
  freeZeroBytes: number | null;
  availableBytes: number | null;
}

export interface ActionRecord {
  id: string;
  timestampMs: number;
  origin: "manual" | "automatic";
  operation: "purge_standby_list";
  outcome: string;
  nativeRequestSucceeded: boolean;
  nativeLifecycle: string;
  elevationUsed: boolean;
  lateCompletionPossible: boolean;
  ntStatusHex: string | null;
  before: ActionMeasurement | null;
  after: ActionMeasurement | null;
  observedReductionBytes: number | null;
  durationMs: number;
  nativeDurationMs: number;
  reason: string;
}

export type PageId = "overview" | "memory" | "performance" | "intelligence" | "settings";
