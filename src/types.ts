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

export interface TelemetryFrame {
  cpuUtilization: Metric;
  snapshot: MemorySnapshot;
  guardian: GuardianDecision;
  cleanerDecision: string;
}

export interface AppSettings {
  performanceHudEnabled: boolean;
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
}

export interface SettingsEnvelope {
  settings: AppSettings;
  warning: string | null;
  path: string;
}

export interface RuntimeStatus {
  paused: boolean;
  cleanerPaused: boolean;
  cleanerDecision: string;
  automaticExecutionReady: boolean;
  actionCapability: ActionCapability;
}

export interface ActionCapability {
  status: "ready" | "requires_elevation" | "confirmed_unsupported" | "unverified" | "mechanism_unavailable";
  compatibilityStatus: "confirmed_supported" | "confirmed_unsupported" | "unverified" | "mechanism_unavailable";
  supported: boolean;
  requiresElevation: boolean;
  privilegeState: string;
  mechanismAvailable: boolean;
  osBuild: number | null;
  mechanism: string;
  explanation: string;
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

export type PageId = "overview" | "explorer" | "guardian" | "settings";

