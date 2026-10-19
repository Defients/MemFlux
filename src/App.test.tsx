import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { AppSettings, SettingsEnvelope } from "./types";
import { SettingsPage } from "./App";

afterEach(cleanup);

const baseSettings = (): AppSettings => ({
  performanceHudEnabled: true,
  schemaVersion: 5,
  guardianEnabled: true,
  closeToTray: false,
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
  cleanerExcludedApplications: []
});

const envelope = (settings: AppSettings): SettingsEnvelope => ({
  settings,
  warning: null,
  path: "C:\\memflux\\settings.json"
});

const guardianToggle = () => screen.getByRole("checkbox", { name: /Adaptive Guardian/ });

test("the save floater stays hidden until a draft diverges, then save and discard settle it", async () => {
  const onSave = vi.fn().mockResolvedValue(undefined);
  const settings = baseSettings();
  const { rerender } = render(<SettingsPage envelope={envelope(settings)} frame={null} saving={false} onSave={onSave} />);

  expect(screen.queryByText("Unsaved changes")).not.toBeInTheDocument();
  expect(screen.queryByText("All changes saved")).not.toBeInTheDocument();

  fireEvent.click(guardianToggle());
  expect(screen.getByText("Unsaved changes")).toBeInTheDocument();

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
  });
  expect(onSave).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ guardianEnabled: false }));

  // The envelope only converges after the native side persists and echoes back.
  rerender(<SettingsPage envelope={envelope({ ...settings, guardianEnabled: false })} frame={null} saving={false} onSave={onSave} />);
  expect(screen.getByText("All changes saved")).toBeInTheDocument();

  cleanup();
  render(<SettingsPage envelope={envelope(settings)} frame={null} saving={false} onSave={onSave} />);
  fireEvent.click(guardianToggle());
  fireEvent.click(screen.getByRole("button", { name: "Discard" }));
  expect(screen.queryByText("Unsaved changes")).not.toBeInTheDocument();
  expect(guardianToggle()).toBeChecked();
  expect(onSave).toHaveBeenCalledTimes(1);
});

test("a new edit during the saved window reports unsaved instead of reusing the saved label", async () => {
  const onSave = vi.fn().mockResolvedValue(undefined);
  const settings = baseSettings();
  const { rerender } = render(<SettingsPage envelope={envelope(settings)} frame={null} saving={false} onSave={onSave} />);

  fireEvent.click(guardianToggle());
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
  });
  rerender(<SettingsPage envelope={envelope({ ...settings, guardianEnabled: false })} frame={null} saving={false} onSave={onSave} />);
  expect(screen.getByText("All changes saved")).toBeInTheDocument();

  fireEvent.click(guardianToggle());
  expect(screen.getByText("Unsaved changes")).toBeInTheDocument();
  expect(screen.queryByText("All changes saved")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save changes" })).toBeEnabled();
});

test("a failed save keeps the unsaved state instead of claiming success", async () => {
  const onSave = vi.fn().mockRejectedValue(new Error("settings are being written by another surface"));
  render(<SettingsPage envelope={envelope(baseSettings())} frame={null} saving={false} onSave={onSave} />);

  fireEvent.click(guardianToggle());
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
  });

  expect(onSave).toHaveBeenCalled();
  expect(screen.getByText("Unsaved changes")).toBeInTheDocument();
  expect(screen.queryByText("All changes saved")).not.toBeInTheDocument();
});

test("the saved confirmation dismisses itself and is cancelled on unmount", async () => {
  vi.useFakeTimers();
  const onSave = vi.fn().mockResolvedValue(undefined);
  const settings = baseSettings();
  const { rerender } = render(<SettingsPage envelope={envelope(settings)} frame={null} saving={false} onSave={onSave} />);

  fireEvent.click(guardianToggle());
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
  });
  rerender(<SettingsPage envelope={envelope({ ...settings, guardianEnabled: false })} frame={null} saving={false} onSave={onSave} />);
  expect(screen.getByText("All changes saved")).toBeInTheDocument();

  act(() => {
    vi.advanceTimersByTime(2000);
  });
  expect(screen.queryByText("All changes saved")).not.toBeInTheDocument();
  vi.useRealTimers();
});
