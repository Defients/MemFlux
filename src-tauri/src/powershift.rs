use crate::rewind::{RewindEntry, RewindJournal};
use serde::{Deserialize, Serialize};

pub const TARGET_WINDOWS_POWER_SCHEME: &str = "windows_active_power_scheme";
pub const GUID_BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub const GUID_HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
pub const GUID_ULTIMATE_PERFORMANCE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
pub const GUID_POWER_SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PowerShiftMode {
    #[default]
    CurrentSystemMode,
    Balanced,
    Performance,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerSchemeInfo {
    pub guid: String,
    pub name: String,
    pub active: bool,
    pub category: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerSupplyStatus {
    pub ac_status: String,
    pub on_battery: bool,
    pub battery_percent: Option<u8>,
    pub battery_saver_active: bool,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerShiftStatus {
    pub supported: bool,
    pub active_scheme: Option<PowerSchemeInfo>,
    pub available_schemes: Vec<PowerSchemeInfo>,
    pub power_supply: PowerSupplyStatus,
    pub memflux_owns_active_change: bool,
    pub active_rewind_entry: Option<RewindEntry>,
    pub performance_scheme_available: bool,
    pub balanced_scheme_available: bool,
    pub capability_note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerShiftOutcome {
    pub action_executed: bool,
    pub system_state_verified: bool,
    pub workload_effect: String,
    pub previous_scheme: Option<PowerSchemeInfo>,
    pub applied_scheme: Option<PowerSchemeInfo>,
    pub rewind_entry: Option<RewindEntry>,
    pub warnings: Vec<String>,
    pub detail: String,
}

pub fn classify_scheme_category(guid: &str, name: &str) -> &'static str {
    let g = guid.to_ascii_lowercase();
    let n = name.to_ascii_lowercase();
    if g == GUID_ULTIMATE_PERFORMANCE
        || g == GUID_HIGH_PERFORMANCE
        || n.contains("high performance")
        || n.contains("ultimate performance")
    {
        "performance"
    } else if g == GUID_BALANCED || n.contains("balanced") {
        "balanced"
    } else if g == GUID_POWER_SAVER || n.contains("power saver") || n.contains("eco") {
        "power_saver"
    } else {
        "custom"
    }
}

pub fn resolve_mode_target_scheme<'a>(
    mode: PowerShiftMode,
    custom_guid: Option<&str>,
    available: &'a [PowerSchemeInfo],
) -> Option<&'a PowerSchemeInfo> {
    match mode {
        PowerShiftMode::CurrentSystemMode => available.iter().find(|s| s.active),
        PowerShiftMode::Balanced => available
            .iter()
            .find(|s| s.guid.eq_ignore_ascii_case(GUID_BALANCED))
            .or_else(|| available.iter().find(|s| s.category == "balanced")),
        PowerShiftMode::Performance => available
            .iter()
            .find(|s| s.guid.eq_ignore_ascii_case(GUID_ULTIMATE_PERFORMANCE))
            .or_else(|| {
                available
                    .iter()
                    .find(|s| s.guid.eq_ignore_ascii_case(GUID_HIGH_PERFORMANCE))
            })
            .or_else(|| available.iter().find(|s| s.category == "performance")),
        PowerShiftMode::Custom => {
            let target = custom_guid?;
            available
                .iter()
                .find(|s| s.guid.eq_ignore_ascii_case(target))
        }
    }
}

pub fn query_status(journal: &RewindJournal) -> PowerShiftStatus {
    let available = platform::enumerate_power_schemes();
    let active = available.iter().find(|s| s.active).cloned();
    let supply = platform::query_power_supply_status();
    let active_rewind = journal.active_for_target(TARGET_WINDOWS_POWER_SCHEME);
    let memflux_owns = active_rewind.as_ref().is_some_and(|entry| {
        active.as_ref().is_some_and(|curr| {
            entry
                .observed_applied_state
                .as_deref()
                .unwrap_or(&entry.intended_new_state)
                .eq_ignore_ascii_case(&curr.guid)
        })
    });
    let performance_available =
        resolve_mode_target_scheme(PowerShiftMode::Performance, None, &available).is_some();
    let balanced_available =
        resolve_mode_target_scheme(PowerShiftMode::Balanced, None, &available).is_some();

    PowerShiftStatus {
        supported: !available.is_empty() && active.is_some(),
        active_scheme: active,
        available_schemes: available,
        power_supply: supply,
        memflux_owns_active_change: memflux_owns,
        active_rewind_entry: active_rewind,
        performance_scheme_available: performance_available,
        balanced_scheme_available: balanced_available,
        capability_note: "Controls traditional Windows PowrProf power schemes (PowerGetActiveScheme / PowerSetActiveScheme). Modern Standby overlay power modes and OEM firmware thermal profiles remain under Windows Settings / OEM control.".into(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_power_mode(
    mode: PowerShiftMode,
    custom_guid: Option<&str>,
    initiating_feature: &str,
    session_id: Option<String>,
    authorization: &str,
    allow_on_battery: bool,
    now_ms: u64,
    journal: &mut RewindJournal,
) -> Result<PowerShiftOutcome, String> {
    let schemes = platform::enumerate_power_schemes();
    let current = schemes.iter().find(|s| s.active).cloned().ok_or_else(|| {
        "Could not determine the current active Windows power scheme.".to_string()
    })?;

    if mode == PowerShiftMode::CurrentSystemMode {
        return Ok(PowerShiftOutcome {
            action_executed: false,
            system_state_verified: true,
            workload_effect: "no_change_requested".into(),
            previous_scheme: Some(current.clone()),
            applied_scheme: Some(current),
            rewind_entry: None,
            warnings: Vec::new(),
            detail: "Current System Mode selected; Windows active power scheme was left unchanged."
                .into(),
        });
    }

    let target = resolve_mode_target_scheme(mode, custom_guid, &schemes)
        .cloned()
        .ok_or_else(|| {
            format!(
                "Requested power mode ({mode:?}) is not available on this Windows installation. MemFlux never fabricates missing power scheme GUIDs."
            )
        })?;

    let supply = platform::query_power_supply_status();
    let mut warnings = Vec::new();
    if let Some(w) = &supply.warning {
        warnings.push(w.clone());
    }
    if supply.on_battery && target.category == "performance" {
        if !allow_on_battery {
            return Err(
                "System is currently running on battery power. High-power scheme activation is blocked by conservative battery policy unless explicitly overridden."
                    .into(),
            );
        }
        warnings.push(
            "Applied performance-oriented power scheme while on battery; increased power draw and heat are expected."
                .into(),
        );
    }

    if current.guid.eq_ignore_ascii_case(&target.guid) {
        return Ok(PowerShiftOutcome {
            action_executed: false,
            system_state_verified: true,
            workload_effect: "already_active".into(),
            previous_scheme: Some(current.clone()),
            applied_scheme: Some(target.clone()),
            rewind_entry: journal.active_for_target(TARGET_WINDOWS_POWER_SCHEME),
            warnings,
            detail: format!(
                "Power scheme '{}' ({}) is already active.",
                target.name, target.guid
            ),
        });
    }

    let pending = journal.record_before_mutation(
        initiating_feature,
        session_id,
        TARGET_WINDOWS_POWER_SCHEME,
        &current.guid,
        &current.name,
        &target.guid,
        &target.name,
        authorization,
        "PowerSetActiveScheme",
        true,
        now_ms,
    )?;

    if let Err(apply_err) = platform::set_active_scheme_guid(&target.guid) {
        // PowerSetActiveScheme returned a Win32 error: no mutation occurred.
        let _ = journal.mark_failed_before_mutation(&pending.action_id, &apply_err);
        return Err(apply_err);
    }

    let verified_guid = match platform::get_active_scheme_guid() {
        Ok(g) => g,
        Err(verify_err) => {
            // The mutation call succeeded; the OS state may have changed but the
            // read-back failed. Keep the restoration obligation recoverable.
            let _ = journal.mark_outcome_uncertain(
                &pending.action_id,
                None,
                &format!("PowerSetActiveScheme succeeded but PowerGetActiveScheme verification failed: {verify_err}"),
            );
            return Err(format!(
                "Power scheme change may have been applied but could not be verified; the change is journaled as outcome-uncertain and remains recoverable. {verify_err}"
            ));
        }
    };

    if !verified_guid.eq_ignore_ascii_case(&target.guid) {
        let msg = format!(
            "PowerSetActiveScheme returned success, but post-change verification reported active GUID '{verified_guid}' instead of requested '{}'.",
            target.guid
        );
        let _ = journal.mark_outcome_uncertain(&pending.action_id, Some(&verified_guid), &msg);
        return Err(msg);
    }

    let confirmed = journal.confirm_mutation_applied(&pending.action_id, &verified_guid)?;
    let mut updated_target = target;
    updated_target.active = true;

    Ok(PowerShiftOutcome {
        action_executed: true,
        system_state_verified: true,
        workload_effect:
            "configuration_verified_no_automatic_fps_claim (compare equivalent frame captures to measure workload effect)"
                .into(),
        previous_scheme: Some(current),
        applied_scheme: Some(updated_target.clone()),
        rewind_entry: Some(confirmed),
        warnings,
        detail: format!(
            "Windows active power scheme changed to '{}' ({}) and verified via PowerGetActiveScheme.",
            updated_target.name, updated_target.guid
        ),
    })
}

pub fn restore_power_scheme(
    now_ms: u64,
    journal: &mut RewindJournal,
) -> Result<Option<RewindEntry>, String> {
    let Some(active) = journal.active_for_target(TARGET_WINDOWS_POWER_SCHEME) else {
        return Ok(None);
    };
    let entry = journal.restore_entry(
        &active.action_id,
        now_ms,
        |_| platform::get_active_scheme_guid(),
        |_, previous_guid| {
            platform::set_active_scheme_guid(previous_guid)?;
            platform::get_active_scheme_guid()
        },
    )?;
    Ok(Some(entry))
}

pub fn read_live_scheme_guid() -> Result<String, String> {
    platform::get_active_scheme_guid()
}

pub fn apply_live_scheme_guid(guid: &str) -> Result<String, String> {
    platform::set_active_scheme_guid(guid)?;
    platform::get_active_scheme_guid()
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{PowerSchemeInfo, PowerSupplyStatus, classify_scheme_category};
    use std::mem::size_of;
    use windows::Win32::Foundation::{
        ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, HLOCAL, LocalFree,
    };
    use windows::Win32::System::Power::{
        ACCESS_SCHEME, GetSystemPowerStatus, PowerEnumerate, PowerGetActiveScheme,
        PowerReadFriendlyName, PowerSetActiveScheme, SYSTEM_POWER_STATUS,
    };
    use windows::core::GUID;

    fn format_guid(guid: &GUID) -> String {
        format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            guid.data1,
            guid.data2,
            guid.data3,
            guid.data4[0],
            guid.data4[1],
            guid.data4[2],
            guid.data4[3],
            guid.data4[4],
            guid.data4[5],
            guid.data4[6],
            guid.data4[7]
        )
    }

    pub fn parse_guid(text: &str) -> Option<GUID> {
        let clean = text.trim().trim_start_matches('{').trim_end_matches('}');
        let parts: Vec<&str> = clean.split('-').collect();
        if parts.len() != 5 {
            return None;
        }
        let d1 = u32::from_str_radix(parts[0], 16).ok()?;
        let d2 = u16::from_str_radix(parts[1], 16).ok()?;
        let d3 = u16::from_str_radix(parts[2], 16).ok()?;
        if parts[3].len() != 4 || parts[4].len() != 12 {
            return None;
        }
        let mut d4 = [0_u8; 8];
        d4[0] = u8::from_str_radix(&parts[3][0..2], 16).ok()?;
        d4[1] = u8::from_str_radix(&parts[3][2..4], 16).ok()?;
        for i in 0..6 {
            d4[2 + i] = u8::from_str_radix(&parts[4][i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(GUID::from_values(d1, d2, d3, d4))
    }

    pub fn get_active_scheme_guid() -> Result<String, String> {
        let mut ptr: *mut GUID = std::ptr::null_mut();
        let err = unsafe { PowerGetActiveScheme(None, &mut ptr) };
        if err != ERROR_SUCCESS || ptr.is_null() {
            return Err(format!(
                "PowerGetActiveScheme failed with Win32 error {}",
                err.0
            ));
        }
        let guid = unsafe { *ptr };
        let _ = unsafe { LocalFree(Some(HLOCAL(ptr.cast()))) };
        Ok(format_guid(&guid))
    }

    pub fn set_active_scheme_guid(guid_str: &str) -> Result<(), String> {
        let guid = parse_guid(guid_str)
            .ok_or_else(|| format!("Invalid power scheme GUID '{guid_str}'"))?;
        let err = unsafe { PowerSetActiveScheme(None, Some(&guid)) };
        if err != ERROR_SUCCESS {
            return Err(format!(
                "PowerSetActiveScheme({guid_str}) failed with Win32 error {}",
                err.0
            ));
        }
        Ok(())
    }

    fn read_scheme_friendly_name(guid: &GUID) -> Option<String> {
        let mut size = 0_u32;
        let first = unsafe { PowerReadFriendlyName(None, Some(guid), None, None, None, &mut size) };
        if (first != ERROR_MORE_DATA && first != ERROR_SUCCESS) || size < 2 {
            return None;
        }
        let mut buffer = vec![0_u8; size as usize];
        let second = unsafe {
            PowerReadFriendlyName(
                None,
                Some(guid),
                None,
                None,
                Some(buffer.as_mut_ptr()),
                &mut size,
            )
        };
        if second != ERROR_SUCCESS {
            return None;
        }
        let words = unsafe {
            std::slice::from_raw_parts(buffer.as_ptr() as *const u16, (size as usize) / 2)
        };
        let trimmed = words.split(|&c| c == 0).next().unwrap_or(words);
        String::from_utf16(trimmed)
            .ok()
            .map(|s| s.trim().to_string())
    }

    pub fn enumerate_power_schemes() -> Vec<PowerSchemeInfo> {
        let active_guid = get_active_scheme_guid().unwrap_or_default();
        let mut schemes = Vec::new();
        let mut index = 0_u32;

        loop {
            let mut guid = GUID::zeroed();
            let mut size = size_of::<GUID>() as u32;
            let err = unsafe {
                PowerEnumerate(
                    None,
                    None,
                    None,
                    ACCESS_SCHEME,
                    index,
                    Some((&mut guid as *mut GUID).cast()),
                    &mut size,
                )
            };
            if err == ERROR_NO_MORE_ITEMS {
                break;
            }
            if err != ERROR_SUCCESS {
                break;
            }
            index += 1;
            let guid_str = format_guid(&guid);
            let name = read_scheme_friendly_name(&guid).unwrap_or_else(|| guid_str.clone());
            let category = classify_scheme_category(&guid_str, &name).to_string();
            let active = guid_str.eq_ignore_ascii_case(&active_guid);
            schemes.push(PowerSchemeInfo {
                guid: guid_str,
                name,
                active,
                category,
            });
        }
        schemes
    }

    pub fn query_power_supply_status() -> PowerSupplyStatus {
        let mut status = SYSTEM_POWER_STATUS::default();
        if unsafe { GetSystemPowerStatus(&mut status) }.is_err() {
            return PowerSupplyStatus {
                ac_status: "unknown".into(),
                on_battery: false,
                battery_percent: None,
                battery_saver_active: false,
                warning: None,
            };
        }
        let on_battery = status.ACLineStatus == 0;
        let ac_status = match status.ACLineStatus {
            0 => "on_battery",
            1 => "ac_online",
            _ => "unknown",
        };
        let battery_percent =
            (status.BatteryLifePercent <= 100).then_some(status.BatteryLifePercent);
        let battery_saver_active = status.SystemStatusFlag == 1;
        let warning = if on_battery {
            Some(
                "Running on battery power. Performance power schemes increase power draw and heat."
                    .into(),
            )
        } else if battery_saver_active {
            Some("Windows Energy Saver is active.".into())
        } else {
            None
        };
        PowerSupplyStatus {
            ac_status: ac_status.into(),
            on_battery,
            battery_percent,
            battery_saver_active,
            warning,
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::{PowerSchemeInfo, PowerSupplyStatus};

    pub fn get_active_scheme_guid() -> Result<String, String> {
        Err("PowerShift is supported on Windows only.".into())
    }
    pub fn set_active_scheme_guid(_guid_str: &str) -> Result<(), String> {
        Err("PowerShift is supported on Windows only.".into())
    }
    pub fn enumerate_power_schemes() -> Vec<PowerSchemeInfo> {
        Vec::new()
    }
    pub fn query_power_supply_status() -> PowerSupplyStatus {
        PowerSupplyStatus {
            ac_status: "unknown".into(),
            on_battery: false,
            battery_percent: None,
            battery_saver_active: false,
            warning: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_power_schemes_enumerate_and_round_trip_restore_with_rewind() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir()
            .join(format!("memflux-powershift-test-{nonce}"))
            .join("rewind-journal.json");
        let mut journal = RewindJournal::load(path.clone());

        let status = query_status(&journal);
        assert!(
            status.supported,
            "Windows power schemes should be enumerated"
        );
        assert!(status.active_scheme.is_some());
        let original = status.active_scheme.clone().expect("active scheme");

        // Find an alternate scheme if one exists to test real activation + verification + Rewind restoration
        if let Some(alternate) = status
            .available_schemes
            .iter()
            .find(|s| !s.guid.eq_ignore_ascii_case(&original.guid))
            .cloned()
        {
            let outcome = apply_power_mode(
                PowerShiftMode::Custom,
                Some(&alternate.guid),
                "powershift_test",
                None,
                "automated_native_verification",
                true,
                1_000,
                &mut journal,
            )
            .expect("apply alternate power scheme");
            assert!(outcome.action_executed);
            assert!(outcome.system_state_verified);
            assert_eq!(
                read_live_scheme_guid()
                    .expect("live guid")
                    .to_ascii_lowercase(),
                alternate.guid.to_ascii_lowercase()
            );

            // Restore immediately and verify original scheme is active again
            let restored = restore_power_scheme(2_000, &mut journal)
                .expect("restore power scheme")
                .expect("rewind entry present");
            assert_eq!(restored.status, crate::rewind::RewindStatus::Restored);
            assert_eq!(
                read_live_scheme_guid()
                    .expect("restored guid")
                    .to_ascii_lowercase(),
                original.guid.to_ascii_lowercase()
            );
        }

        let _ = std::fs::remove_file(&path);
        if let Some(p) = path.parent() {
            let _ = std::fs::remove_dir(p);
        }
    }
}
