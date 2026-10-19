use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const REWIND_SCHEMA_VERSION: u32 = 1;
const REWIND_JOURNAL_LIMIT: usize = 200;
static REWIND_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewindStatus {
    Pending,
    Active,
    Restored,
    ConflictSkipped,
    Failed,
    Irreversible,
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

    pub fn active_entries(&self) -> Vec<RewindEntry> {
        self.entries
            .iter()
            .filter(|e| e.status == RewindStatus::Active)
            .cloned()
            .collect()
    }

    pub fn active_for_target(&self, target_identity: &str) -> Option<RewindEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.target_identity == target_identity && e.status == RewindStatus::Active)
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
        // If there is already an Active entry for the same target, preserve its original previous_state
        // so chained GameFocus/PowerShift changes restore back to the user's true original baseline.
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
                RewindStatus::Pending
            } else {
                RewindStatus::Irreversible
            },
            error_detail: None,
        };
        if self.entries.len() >= REWIND_JOURNAL_LIMIT {
            self.entries.pop_front();
        }
        self.entries.push_back(entry.clone());
        self.persist()?;
        Ok(entry)
    }

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

        // Supersede older Active entries for the same target since the new entry carries the original baseline
        let now = crate::telemetry::timestamp_ms();
        for item in &mut self.entries {
            if item.target_identity == target_identity
                && item.action_id != action_id
                && item.status == RewindStatus::Active
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
            entry.status = RewindStatus::Active;
        }
        let cloned = entry.clone();
        self.persist()?;
        Ok(cloned)
    }

    pub fn mark_mutation_failed(
        &mut self,
        action_id: &str,
        error: &str,
    ) -> Result<RewindEntry, String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.action_id == action_id)
            .ok_or_else(|| format!("Rewind entry '{action_id}' not found."))?;
        entry.status = RewindStatus::Failed;
        entry.error_detail = Some(error.to_owned());
        let cloned = entry.clone();
        self.persist()?;
        Ok(cloned)
    }

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
        if current_entry.status == RewindStatus::Restored {
            return Ok(current_entry);
        }

        let live_state = match read_current_state(&current_entry.target_identity) {
            Ok(state) => state,
            Err(error) => {
                self.entries[idx].status = RewindStatus::Failed;
                self.entries[idx].error_detail = Some(format!(
                    "Could not inspect current state before restore: {error}"
                ));
                let out = self.entries[idx].clone();
                let _ = self.persist();
                return Ok(out);
            }
        };

        // Idempotent check: if the live state already matches previous_state, mark Restored
        if live_state.eq_ignore_ascii_case(&current_entry.previous_state) {
            self.entries[idx].status = RewindStatus::Restored;
            self.entries[idx].restored_timestamp_ms = Some(now_ms);
            self.entries[idx].error_detail = None;
            let out = self.entries[idx].clone();
            self.persist()?;
            return Ok(out);
        }

        // Compare-before-restore: if live state no longer matches what MemFlux applied,
        // the user or OS changed it externally. Never overwrite external user selection!
        let expected_applied = current_entry
            .observed_applied_state
            .as_deref()
            .unwrap_or(&current_entry.intended_new_state);
        if !live_state.eq_ignore_ascii_case(expected_applied) {
            self.entries[idx].status = RewindStatus::ConflictSkipped;
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
                self.entries[idx].status = RewindStatus::Failed;
                self.entries[idx].error_detail = Some(format!(
                    "Restore command ran, but observed state '{}' did not match expected '{}'.",
                    observed_after_restore, current_entry.previous_state
                ));
            }
            Err(error) => {
                self.entries[idx].status = RewindStatus::Failed;
                self.entries[idx].error_detail = Some(error);
            }
        }

        let out = self.entries[idx].clone();
        self.persist()?;
        Ok(out)
    }

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
            .filter(|e| e.status == RewindStatus::Active && e.reversible)
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

    pub fn recover_on_startup<R, A>(
        &mut self,
        now_ms: u64,
        mut read_current_state: R,
        mut apply_restore: A,
    ) -> Vec<RewindEntry>
    where
        R: FnMut(&str) -> Result<String, String>,
        A: FnMut(&str, &str) -> Result<String, String>,
    {
        // Clean up any incomplete Pending entries that never reached Active before a crash
        for entry in &mut self.entries {
            if entry.status == RewindStatus::Pending {
                entry.status = RewindStatus::Failed;
                entry.error_detail = Some(
                    "Entry was pending when the previous session exited; marked failed without blind mutation."
                        .into(),
                );
            }
        }
        let recovered =
            self.restore_all_eligible(now_ms, &mut read_current_state, &mut apply_restore);
        let _ = self.persist();
        recovered
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
        Ok(mut file) if file.schema_version == REWIND_SCHEMA_VERSION => {
            while file.entries.len() > REWIND_JOURNAL_LIMIT {
                file.entries.pop_front();
            }
            file.entries
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

        let _ = fs::remove_file(&path);
        if let Some(p) = path.parent() {
            let _ = fs::remove_dir(p);
        }
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

        assert_eq!(outcome.status, RewindStatus::ConflictSkipped);
        assert_eq!(*state.borrow(), "scheme-c");
        assert!(
            outcome
                .error_detail
                .unwrap_or_default()
                .contains("External change detected")
        );

        let _ = fs::remove_file(&path);
        if let Some(p) = path.parent() {
            let _ = fs::remove_dir(p);
        }
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

        // Simulate restart by loading from disk and running recover_on_startup
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

        let _ = fs::remove_file(&path);
        if let Some(p) = path.parent() {
            let _ = fs::remove_dir(p);
        }
    }
}
