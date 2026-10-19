use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const REWIND_SCHEMA_VERSION: u32 = 2;
const REWIND_JOURNAL_LIMIT: usize = 200;
static REWIND_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Optimization Rewind V2 lifecycle.
///
/// Every reversible mutation moves through the obligation side
/// (`prepared` -> `applied_verified` | `outcome_uncertain` | `failed_before_mutation`)
/// and the restoration side (`restoration_pending` -> `restored` |
/// `external_change_preserved`). `recovery_required` marks entries that still
/// need reconciliation but could not be inspected. Any non-terminal state
/// retains its restoration obligation and is never evicted by retention limits.
///
/// Schema v1 names are accepted on load through serde aliases:
/// `pending` -> `prepared`, `active` -> `applied_verified`,
/// `failed` -> `outcome_uncertain`, `conflict_skipped` -> `external_change_preserved`.
/// Legacy `failed` entries must map to `outcome_uncertain` (not
/// `failed_before_mutation`) because v1 could not distinguish whether the OS
/// mutation had already run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewindStatus {
    /// Intent persisted; the native mutation has not yet been confirmed.
    #[serde(alias = "pending")]
    Prepared,
    /// Mutation confirmed applied; restoration obligation is live.
    #[serde(alias = "active")]
    AppliedVerified,
    /// The mutation may have been applied but verification could not prove it.
    #[serde(alias = "failed")]
    OutcomeUncertain,
    /// Restoration is needed but an attempt failed or was interrupted.
    RestorationPending,
    /// Live state matches the recorded baseline (by restore or reconciliation).
    Restored,
    /// Live state no longer matches what MemFlux applied; preserved external choice.
    #[serde(alias = "conflict_skipped")]
    ExternalChangePreserved,
    /// Startup recovery could not inspect or resolve this entry.
    RecoveryRequired,
    /// The mutation call itself failed; no system mutation occurred.
    FailedBeforeMutation,
    /// Recorded action is not reversible.
    Irreversible,
}

impl RewindStatus {
    /// True while the entry still carries a restoration obligation.
    pub fn is_unresolved(self) -> bool {
        matches!(
            self,
            RewindStatus::Prepared
                | RewindStatus::AppliedVerified
                | RewindStatus::OutcomeUncertain
                | RewindStatus::RestorationPending
                | RewindStatus::RecoveryRequired
        )
    }

    pub fn is_terminal(self) -> bool {
        !self.is_unresolved()
    }

    /// Human-facing label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            RewindStatus::Prepared => "prepared",
            RewindStatus::AppliedVerified => "applied (verified)",
            RewindStatus::OutcomeUncertain => "outcome uncertain",
            RewindStatus::RestorationPending => "restoration pending",
            RewindStatus::Restored => "restored",
            RewindStatus::ExternalChangePreserved => "external change preserved",
            RewindStatus::RecoveryRequired => "recovery required",
            RewindStatus::FailedBeforeMutation => "failed (no change applied)",
            RewindStatus::Irreversible => "irreversible",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewindEntry {
    pub action_id: String,
    pub initiating_feature: String,
    pub session_id: Option<String>,
    pub target_identity: String,
    pub previous_state: String,
    pub previous_state_label: String,
    pub intended_new_state: String,
    pub intended_new_state_label: String,
    pub observed_applied_state: Option<String>,
    pub timestamp_ms: u64,
    pub restored_timestamp_ms: Option<u64>,
    pub authorization: String,
    pub restoration_method: String,
    pub reversible: bool,
    pub status: RewindStatus,
    pub error_detail: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RewindJournalFile {
    schema_version: u32,
    entries: VecDeque<RewindEntry>,
}

pub struct RewindJournal {
    path: PathBuf,
    entries: VecDeque<RewindEntry>,
}

impl RewindJournal {
    pub fn load(path: PathBuf) -> Self {
        let entries = load_entries(&path);
        Self { path, entries }
    }

    pub fn entries(&self) -> Vec<RewindEntry> {
        self.entries.iter().cloned().collect()
    }

    /// Entries that still carry a live restoration obligation and are
    /// reversible. Includes verified-active changes *and* uncertain ones.
    pub fn active_entries(&self) -> Vec<RewindEntry> {
        self.entries
            .iter()
            .filter(|e| e.reversible && e.status.is_unresolved())
            .cloned()
            .collect()
    }

    /// Newest unresolved reversible entry for a target. Used for baseline
    /// chaining so repeated changes restore to the user's true original state.
    pub fn active_for_target(&self, target_identity: &str) -> Option<RewindEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| {
                e.target_identity == target_identity && e.reversible && e.status.is_unresolved()
            })
            .cloned()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_before_mutation(
        &mut self,
        initiating_feature: &str,
        session_id: Option<String>,
        target_identity: &str,
        previous_state: &str,
        previous_state_label: &str,
        intended_new_state: &str,
        intended_new_state_label: &str,
        authorization: &str,
        restoration_method: &str,
        reversible: bool,
        timestamp_ms: u64,
    ) -> Result<RewindEntry, String> {
        // If there is already an unresolved entry for the same target, preserve its
        // original previous_state so chained changes restore to the true baseline.
        let (baseline_state, baseline_label) =
            if let Some(existing) = self.active_for_target(target_identity) {
                (existing.previous_state, existing.previous_state_label)
            } else {
                (previous_state.to_owned(), previous_state_label.to_owned())
            };

        let seq = REWIND_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let entry = RewindEntry {
            action_id: format!("rewind-{timestamp_ms}-{seq}"),
            initiating_feature: initiating_feature.to_owned(),
            session_id,
            target_identity: target_identity.to_owned(),
            previous_state: baseline_state,
            previous_state_label: baseline_label,
            intended_new_state: intended_new_state.to_owned(),
            intended_new_state_label: intended_new_state_label.to_owned(),
            observed_applied_state: None,
            timestamp_ms,
            restored_timestamp_ms: None,
            authorization: authorization.to_owned(),
            restoration_method: restoration_method.to_owned(),
            reversible,
            status: if reversible {
                RewindStatus::Prepared
            } else {
                RewindStatus::Irreversible
            },
            error_detail: None,
        };
        self.entries.push_back(entry.clone());
        self.enforce_limit();
        self.persist()?;
        Ok(entry)
    }

    /// The native mutation was verified applied. Carries the observed state.
    pub fn confirm_mutation_applied(
        &mut self,
        action_id: &str,
        observed_applied_state: &str,
    ) -> Result<RewindEntry, String> {
        let target_identity = self
            .entries
            .iter()
            .find(|e| e.action_id == action_id)
            .map(|e| e.target_identity.clone())
            .ok_or_else(|| format!("Rewind entry '{action_id}' not found."))?;

        // Supersede older unresolved entries for the same target; the new entry
        // retains the original baseline.
        let now = crate::telemetry::timestamp_ms();
        for item in &mut self.entries {
            if item.target_identity == target_identity
                && item.action_id != action_id
                && item.reversible
                && item.status.is_unresolved()
            {
                item.status = RewindStatus::Restored;
                item.restored_timestamp_ms = Some(now);
                item.error_detail = Some(format!(
                    "Superseded by newer action {action_id} retaining baseline."
                ));
            }
        }

        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.action_id == action_id)
            .ok_or_else(|| format!("Rewind entry '{action_id}' not found."))?;
        entry.observed_applied_state = Some(observed_applied_state.to_owned());
        if entry.reversible {
            entry.status = RewindStatus::AppliedVerified;
        }
        let cloned = entry.clone();
        self.persist()?;
        Ok(cloned)
    }

    /// The mutation call itself failed before any system mutation. Only use
    /// when it is *known* that no OS change occurred.
    pub fn mark_failed_before_mutation(
        &mut self,
        action_id: &str,
        error: &str,
    ) -> Result<RewindEntry, String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.action_id == action_id)
            .ok_or_else(|| format!("Rewind entry '{action_id}' not found."))?;
        entry.status = RewindStatus::FailedBeforeMutation;
        entry.error_detail = Some(error.to_owned());
        let cloned = entry.clone();
        self.persist()?;
        Ok(cloned)
    }

    /// The mutation may have succeeded but verification (or a later journal
    /// step) failed. The entry keeps its restoration obligation; optionally
    /// records the state actually observed after the mutation.
    pub fn mark_outcome_uncertain(
        &mut self,
        action_id: &str,
        observed_state: Option<&str>,
        error: &str,
    ) -> Result<RewindEntry, String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.action_id == action_id)
            .ok_or_else(|| format!("Rewind entry '{action_id}' not found."))?;
        entry.status = RewindStatus::OutcomeUncertain;
        if let Some(observed) = observed_state {
            entry.observed_applied_state = Some(observed.to_owned());
        }
        entry.error_detail = Some(format!("Outcome uncertain: {error}"));
        let cloned = entry.clone();
        self.persist()?;
        Ok(cloned)
    }

    /// Evaluate one entry against live state and restore it if MemFlux's
    /// change is still active. Reconciles `prepared` and `outcome_uncertain`
    /// entries via the live read — no blind mutation or blind failure.
    pub fn restore_entry<R, A>(
        &mut self,
        action_id: &str,
        now_ms: u64,
        mut read_current_state: R,
        mut apply_restore: A,
    ) -> Result<RewindEntry, String>
    where
        R: FnMut(&str) -> Result<String, String>,
        A: FnMut(&str, &str) -> Result<String, String>,
    {
        let idx = self
            .entries
            .iter()
            .position(|e| e.action_id == action_id)
            .ok_or_else(|| format!("Rewind entry '{action_id}' not found."))?;

        let current_entry = self.entries[idx].clone();
        if !current_entry.reversible || current_entry.status == RewindStatus::Irreversible {
            return Err(
                "This action is marked irreversible and cannot be automatically restored.".into(),
            );
        }
        if current_entry.status.is_terminal() {
            return Ok(current_entry);
        }

        let live_state = match read_current_state(&current_entry.target_identity) {
            Ok(state) => state,
            Err(error) => {
                // We could not inspect the system; obligation must be recoverable later.
                self.entries[idx].status = RewindStatus::RecoveryRequired;
                self.entries[idx].error_detail = Some(format!(
                    "Could not inspect current state before restore: {error}"
                ));
                let out = self.entries[idx].clone();
                let _ = self.persist();
                return Ok(out);
            }
        };

        // Idempotent: live state already equals the recorded baseline.
        if live_state.eq_ignore_ascii_case(&current_entry.previous_state) {
            self.entries[idx].status = RewindStatus::Restored;
            self.entries[idx].restored_timestamp_ms = Some(now_ms);
            self.entries[idx].error_detail = None;
            let out = self.entries[idx].clone();
            self.persist()?;
            return Ok(out);
        }

        // Compare-before-restore: if live state no longer matches what MemFlux
        // applied (or intended), an external actor changed it — never overwrite.
        let expected_applied = current_entry
            .observed_applied_state
            .as_deref()
            .unwrap_or(&current_entry.intended_new_state);
        if !live_state.eq_ignore_ascii_case(expected_applied) {
            self.entries[idx].status = RewindStatus::ExternalChangePreserved;
            self.entries[idx].restored_timestamp_ms = Some(now_ms);
            self.entries[idx].error_detail = Some(format!(
                "External change detected: current state '{}' differs from MemFlux's applied state '{}'. Preserved external configuration instead of overwriting with '{}'.",
                live_state, expected_applied, current_entry.previous_state
            ));
            let out = self.entries[idx].clone();
            self.persist()?;
            return Ok(out);
        }

        match apply_restore(
            &current_entry.target_identity,
            &current_entry.previous_state,
        ) {
            Ok(observed_after_restore)
                if observed_after_restore.eq_ignore_ascii_case(&current_entry.previous_state) =>
            {
                self.entries[idx].status = RewindStatus::Restored;
                self.entries[idx].restored_timestamp_ms = Some(now_ms);
                self.entries[idx].error_detail = None;
            }
            Ok(observed_after_restore) => {
                self.entries[idx].status = RewindStatus::RestorationPending;
                self.entries[idx].error_detail = Some(format!(
                    "Restore ran, but observed state '{observed_after_restore}' did not match expected '{}'; obligation retained for retry.",
                    current_entry.previous_state
                ));
            }
            Err(error) => {
                self.entries[idx].status = RewindStatus::RestorationPending;
                self.entries[idx].error_detail = Some(format!(
                    "Restore attempt failed; obligation retained for retry: {error}"
                ));
            }
        }

        let out = self.entries[idx].clone();
        self.persist()?;
        Ok(out)
    }

    /// Restore every unresolved reversible entry, newest-first.
    pub fn restore_all_eligible<R, A>(
        &mut self,
        now_ms: u64,
        mut read_current_state: R,
        mut apply_restore: A,
    ) -> Vec<RewindEntry>
    where
        R: FnMut(&str) -> Result<String, String>,
        A: FnMut(&str, &str) -> Result<String, String>,
    {
        let ids: Vec<String> = self
            .entries
            .iter()
            .rev()
            .filter(|e| e.reversible && e.status.is_unresolved())
            .map(|e| e.action_id.clone())
            .collect();
        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            if let Ok(entry) =
                self.restore_entry(&id, now_ms, &mut read_current_state, &mut apply_restore)
            {
                results.push(entry);
            }
        }
        results
    }

    /// Startup reconciliation: every unresolved entry is inspected against live
    /// state. Entries whose live state already matches the baseline resolve to
    /// `restored`; live state matching the applied state is restored; external
    /// changes are preserved; uninspectable entries become `recovery_required`.
    /// Unlike v1, `prepared` entries are reconciled — never blind-failed.
    pub fn recover_on_startup<R, A>(
        &mut self,
        now_ms: u64,
        read_current_state: R,
        apply_restore: A,
    ) -> Vec<RewindEntry>
    where
        R: FnMut(&str) -> Result<String, String>,
        A: FnMut(&str, &str) -> Result<String, String>,
    {
        let recovered = self.restore_all_eligible(now_ms, read_current_state, apply_restore);
        let _ = self.persist();
        recovered
    }

    /// Never evict an unresolved restoration obligation to satisfy retention.
    fn enforce_limit(&mut self) {
        while self.entries.len() > REWIND_JOURNAL_LIMIT {
            match self.entries.iter().position(|e| e.status.is_terminal()) {
                Some(pos) => {
                    self.entries.remove(pos);
                }
                None => {
                    log::warn!(
                        "Rewind journal holds more than {REWIND_JOURNAL_LIMIT} unresolved obligations; retaining all until resolved."
                    );
                    break;
                }
            }
        }
    }

    fn persist(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Rewind journal path has no parent directory.".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create rewind journal directory: {e}"))?;
        let temp = self.path.with_extension("json.new");
        let payload = serde_json::to_vec_pretty(&RewindJournalFile {
            schema_version: REWIND_SCHEMA_VERSION,
            entries: self.entries.clone(),
        })
        .map_err(|e| format!("Could not serialize rewind journal: {e}"))?;
        let mut file = File::create(&temp)
            .map_err(|e| format!("Could not create temporary rewind journal: {e}"))?;
        file.write_all(&payload)
            .map_err(|e| format!("Could not write temporary rewind journal: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Could not flush temporary rewind journal: {e}"))?;
        drop(file);
        crate::settings::atomic_replace(&temp, &self.path)
            .map_err(|e| format!("Could not atomically replace rewind journal: {e}"))
    }
}

fn load_entries(path: &Path) -> VecDeque<RewindEntry> {
    let Ok(raw) = fs::read_to_string(path) else {
        return VecDeque::new();
    };
    match serde_json::from_str::<RewindJournalFile>(&raw) {
        // Schema 1 entries deserialize through status aliases into v2 states.
        Ok(file) if file.schema_version == 1 || file.schema_version == REWIND_SCHEMA_VERSION => {
            let mut journal_entries = file.entries;
            // Enforce limit on load without dropping unresolved obligations.
            while journal_entries.len() > REWIND_JOURNAL_LIMIT {
                match journal_entries.iter().position(|e| e.status.is_terminal()) {
                    Some(pos) => {
                        journal_entries.remove(pos);
                    }
                    None => break,
                }
            }
            journal_entries
        }
        _ => {
            log::warn!(
                "Optimization Rewind journal is corrupt or uses an unsupported schema; preserving file and starting clean."
            );
            VecDeque::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_journal_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("memflux-rewind-test-{label}-{nonce}"))
            .join("rewind-journal.json")
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(path.with_extension("json.new"));
        if let Some(p) = path.parent() {
            let _ = fs::remove_dir(p);
        }
    }

    #[test]
    fn normal_restoration_and_idempotency_work() {
        let path = temp_journal_path("normal");
        let mut journal = RewindJournal::load(path.clone());
        let state = RefCell::new("scheme-a".to_string());

        let pending = journal
            .record_before_mutation(
                "powershift",
                Some("sess-1".into()),
                "windows_active_power_scheme",
                "scheme-a",
                "Balanced",
                "scheme-b",
                "High performance",
                "user_explicit",
                "PowerSetActiveScheme",
                true,
                1_000,
            )
            .expect("record pending");

        *state.borrow_mut() = "scheme-b".into();
        journal
            .confirm_mutation_applied(&pending.action_id, "scheme-b")
            .expect("confirm applied");

        let restored = journal
            .restore_entry(
                &pending.action_id,
                2_000,
                |_| Ok(state.borrow().clone()),
                |_, target| {
                    *state.borrow_mut() = target.to_owned();
                    Ok(target.to_owned())
                },
            )
            .expect("restore");
        assert_eq!(restored.status, RewindStatus::Restored);
        assert_eq!(*state.borrow(), "scheme-a");

        // Repeating restore is idempotent
        let repeat = journal
            .restore_entry(
                &pending.action_id,
                3_000,
                |_| Ok(state.borrow().clone()),
                |_, _| panic!("should not re-invoke apply"),
            )
            .expect("repeat restore");
        assert_eq!(repeat.status, RewindStatus::Restored);
        cleanup(&path);
    }

    #[test]
    fn compare_before_restore_respects_external_changes() {
        let path = temp_journal_path("conflict");
        let mut journal = RewindJournal::load(path.clone());
        let state = RefCell::new("scheme-a".to_string());

        let pending = journal
            .record_before_mutation(
                "gamefocus",
                Some("sess-2".into()),
                "windows_active_power_scheme",
                "scheme-a",
                "Balanced",
                "scheme-b",
                "High performance",
                "gamefocus_profile",
                "PowerSetActiveScheme",
                true,
                1_000,
            )
            .expect("record pending");

        *state.borrow_mut() = "scheme-b".into();
        journal
            .confirm_mutation_applied(&pending.action_id, "scheme-b")
            .expect("confirm applied");

        // User externally changes power scheme to scheme-c
        *state.borrow_mut() = "scheme-c".into();

        let outcome = journal
            .restore_entry(
                &pending.action_id,
                2_000,
                |_| Ok(state.borrow().clone()),
                |_, _| panic!("must not overwrite external scheme-c"),
            )
            .expect("conflict check");

        assert_eq!(outcome.status, RewindStatus::ExternalChangePreserved);
        assert_eq!(*state.borrow(), "scheme-c");
        assert!(
            outcome
                .error_detail
                .unwrap_or_default()
                .contains("External change detected")
        );
        cleanup(&path);
    }

    #[test]
    fn crash_recovery_restores_active_entries_on_startup() {
        let path = temp_journal_path("crash");
        let state = RefCell::new("scheme-b".to_string());
        {
            let mut journal = RewindJournal::load(path.clone());
            let pending = journal
                .record_before_mutation(
                    "powershift",
                    None,
                    "windows_active_power_scheme",
                    "scheme-a",
                    "Balanced",
                    "scheme-b",
                    "High performance",
                    "user_explicit",
                    "PowerSetActiveScheme",
                    true,
                    1_000,
                )
                .expect("record");
            journal
                .confirm_mutation_applied(&pending.action_id, "scheme-b")
                .expect("confirm");
        }

        let mut reloaded = RewindJournal::load(path.clone());
        assert_eq!(reloaded.active_entries().len(), 1);
        let recovered = reloaded.recover_on_startup(
            5_000,
            |_| Ok(state.borrow().clone()),
            |_, target| {
                *state.borrow_mut() = target.to_owned();
                Ok(target.to_owned())
            },
        );
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, RewindStatus::Restored);
        assert_eq!(*state.borrow(), "scheme-a");
        cleanup(&path);
    }

    #[test]
    fn unconfirmed_mutation_recovers_via_live_state_inspection() {
        // Simulates crash between a successful OS mutation and journal confirm:
        // the entry stays `prepared` on disk while live state shows the change.
        let path = temp_journal_path("uncertain-crash");
        let state = RefCell::new("scheme-a".to_string());
        {
            let mut journal = RewindJournal::load(path.clone());
            journal
                .record_before_mutation(
                    "powershift",
                    None,
                    "windows_active_power_scheme",
                    "scheme-a",
                    "Balanced",
                    "scheme-b",
                    "High performance",
                    "user_explicit",
                    "PowerSetActiveScheme",
                    true,
                    1_000,
                )
                .expect("record");
            // Mutation actually happened but confirm_mutation_applied never ran.
            *state.borrow_mut() = "scheme-b".into();
        }

        let mut reloaded = RewindJournal::load(path.clone());
        let recovered = reloaded.recover_on_startup(
            5_000,
            |_| Ok(state.borrow().clone()),
            |_, target| {
                *state.borrow_mut() = target.to_owned();
                Ok(target.to_owned())
            },
        );
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, RewindStatus::Restored);
        assert_eq!(*state.borrow(), "scheme-a");
        cleanup(&path);
    }

    #[test]
    fn outcome_uncertain_entries_are_restored_when_change_is_live() {
        let path = temp_journal_path("uncertain-restore");
        let state = RefCell::new("scheme-b".to_string());
        let mut journal = RewindJournal::load(path.clone());
        let pending = journal
            .record_before_mutation(
                "powershift",
                None,
                "windows_active_power_scheme",
                "scheme-a",
                "Balanced",
                "scheme-b",
                "High performance",
                "user_explicit",
                "PowerSetActiveScheme",
                true,
                1_000,
            )
            .expect("record");
        journal
            .mark_outcome_uncertain(&pending.action_id, Some("scheme-b"), "verification failed")
            .expect("mark uncertain");

        let recovered = journal.restore_entry(
            &pending.action_id,
            2_000,
            |_| Ok(state.borrow().clone()),
            |_, target| {
                *state.borrow_mut() = target.to_owned();
                Ok(target.to_owned())
            },
        );
        assert_eq!(recovered.expect("restore").status, RewindStatus::Restored);
        assert_eq!(*state.borrow(), "scheme-a");
        cleanup(&path);
    }

    #[test]
    fn failed_before_mutation_entries_are_not_restored() {
        let path = temp_journal_path("no-mutation");
        let state = RefCell::new("scheme-a".to_string());
        let mut journal = RewindJournal::load(path.clone());
        let pending = journal
            .record_before_mutation(
                "powershift",
                None,
                "windows_active_power_scheme",
                "scheme-a",
                "Balanced",
                "scheme-b",
                "High performance",
                "user_explicit",
                "PowerSetActiveScheme",
                true,
                1_000,
            )
            .expect("record");
        journal
            .mark_failed_before_mutation(&pending.action_id, "PowerSetActiveScheme returned error")
            .expect("mark failed");

        // Startup recovery must not touch this entry at all.
        let recovered = journal.recover_on_startup(
            5_000,
            |_| Ok(state.borrow().clone()),
            |_, _| panic!("must not attempt restore"),
        );
        assert!(recovered.is_empty());
        let stored = journal
            .entries()
            .into_iter()
            .find(|e| e.action_id == pending.action_id)
            .expect("entry");
        assert_eq!(stored.status, RewindStatus::FailedBeforeMutation);
        cleanup(&path);
    }

    #[test]
    fn restore_failure_keeps_obligation_pending_for_retry() {
        let path = temp_journal_path("pending");
        let state = RefCell::new("scheme-b".to_string());
        let mut journal = RewindJournal::load(path.clone());
        let pending = journal
            .record_before_mutation(
                "powershift",
                None,
                "windows_active_power_scheme",
                "scheme-a",
                "Balanced",
                "scheme-b",
                "High performance",
                "user_explicit",
                "PowerSetActiveScheme",
                true,
                1_000,
            )
            .expect("record");
        journal
            .confirm_mutation_applied(&pending.action_id, "scheme-b")
            .expect("confirm");

        let first = journal
            .restore_entry(
                &pending.action_id,
                2_000,
                |_| Ok(state.borrow().clone()),
                |_, _| Err("simulated transient failure".into()),
            )
            .expect("restore attempt");
        assert_eq!(first.status, RewindStatus::RestorationPending);
        assert!(first.status.is_unresolved());

        // Retry succeeds once the failure clears.
        let second = journal
            .restore_entry(
                &pending.action_id,
                3_000,
                |_| Ok(state.borrow().clone()),
                |_, target| {
                    *state.borrow_mut() = target.to_owned();
                    Ok(target.to_owned())
                },
            )
            .expect("retry");
        assert_eq!(second.status, RewindStatus::Restored);
        cleanup(&path);
    }

    #[test]
    fn unresolved_obligations_survive_retention_limit() {
        let path = temp_journal_path("retention");
        let mut journal = RewindJournal::load(path.clone());
        // Fill with unresolved entries beyond the limit.
        for i in 0..(REWIND_JOURNAL_LIMIT + 10) {
            journal
                .record_before_mutation(
                    "test",
                    None,
                    &format!("target_{i}"),
                    "a",
                    "A",
                    "b",
                    "B",
                    "test",
                    "test",
                    true,
                    i as u64,
                )
                .expect("record");
        }
        let entries = journal.entries();
        assert!(entries.len() > REWIND_JOURNAL_LIMIT);
        assert!(entries.iter().all(|e| e.status.is_unresolved()));
        cleanup(&path);
    }

    #[test]
    fn terminal_entries_are_evicted_before_unresolved() {
        let path = temp_journal_path("retention-order");
        let mut journal = RewindJournal::load(path.clone());
        let unresolved = journal
            .record_before_mutation(
                "test", None, "keep-me", "a", "A", "b", "B", "test", "test", true, 1,
            )
            .expect("record unresolved");
        for i in 0..(REWIND_JOURNAL_LIMIT + 5) {
            let e = journal
                .record_before_mutation(
                    "test",
                    None,
                    &format!("terminal_{i}"),
                    "a",
                    "A",
                    "b",
                    "B",
                    "test",
                    "test",
                    true,
                    10 + i as u64,
                )
                .expect("record");
            journal
                .mark_failed_before_mutation(&e.action_id, "no mutation")
                .expect("mark terminal");
        }
        let entries = journal.entries();
        assert!(entries.len() <= REWIND_JOURNAL_LIMIT);
        assert!(entries.iter().any(|e| e.action_id == unresolved.action_id));
        cleanup(&path);
    }

    #[test]
    fn schema_v1_journal_migrates_failed_to_outcome_uncertain() {
        let path = temp_journal_path("v1-migration");
        let v1 = serde_json::json!({
            "schemaVersion": 1,
            "entries": [{
                "actionId": "rewind-1-1",
                "initiatingFeature": "powershift",
                "sessionId": null,
                "targetIdentity": "windows_active_power_scheme",
                "previousState": "scheme-a",
                "previousStateLabel": "Balanced",
                "intendedNewState": "scheme-b",
                "intendedNewStateLabel": "High performance",
                "observedAppliedState": null,
                "timestampMs": 1000,
                "restoredTimestampMs": null,
                "authorization": "user_explicit",
                "restorationMethod": "PowerSetActiveScheme",
                "reversible": true,
                "status": "failed",
                "errorDetail": "legacy failure"
            }]
        });
        fs::create_dir_all(path.parent().expect("dir")).expect("mkdir");
        fs::write(&path, serde_json::to_string(&v1).expect("json")).expect("write v1");

        let journal = RewindJournal::load(path.clone());
        let entries = journal.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, RewindStatus::OutcomeUncertain);
        assert!(entries[0].status.is_unresolved());
        cleanup(&path);
    }
}
