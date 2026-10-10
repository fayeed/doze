//! Stay awake with the lid closed. Power requests do not stop the lid-close action, so while
//! Doze keeps the PC awake it sets the active power plan's "When I close the lid" to Do
//! nothing, as Settings › System › Power does, without administrator rights. The user's own
//! values are written to a file first, so they come back when Doze lets go, when it quits, and
//! on the next launch after a crash or power loss.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use windows::{
    core::GUID,
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        System::{
            Power::{
                GetPwrCapabilities, PowerGetActiveScheme, PowerReadACValueIndex,
                PowerReadDCValueIndex, PowerSetActiveScheme, PowerWriteACValueIndex,
                PowerWriteDCValueIndex, SYSTEM_POWER_CAPABILITIES,
            },
            Registry::HKEY,
        },
    },
};

/// Power settings › Power buttons and lid › Lid close action (winnt.h).
const GUID_SYSTEM_BUTTON_SUBGROUP: GUID = GUID::from_u128(0x4f971e89_eebd_4455_a8de_9e59040e7347);
const GUID_LIDCLOSE_ACTION: GUID = GUID::from_u128(0x5ca83367_6e45_459f_a27b_476b1d01c936);
/// "Do nothing" for the lid-close action.
const DO_NOTHING: u32 = 0;

/// Whether this PC has a lid.
pub fn supported() -> bool {
    let mut caps = SYSTEM_POWER_CAPABILITIES::default();
    unsafe { GetPwrCapabilities(&mut caps) }.as_bool() && caps.LidPresent.as_bool()
}

/// The user's lid-close actions on a power plan, to put back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Saved {
    scheme: u128,
    ac: u32,
    dc: u32,
}

fn active_scheme() -> Result<GUID, String> {
    unsafe {
        let mut pointer: *mut GUID = std::ptr::null_mut();
        PowerGetActiveScheme(HKEY::default(), &mut pointer)
            .ok()
            .map_err(|e| format!("Could not read the power plan: {e}"))?;
        if pointer.is_null() {
            return Err("Could not read the power plan.".into());
        }
        let scheme = *pointer;
        let _ = LocalFree(HLOCAL(pointer.cast()));
        Ok(scheme)
    }
}

fn read(scheme: &GUID) -> Result<(u32, u32), String> {
    let (mut ac, mut dc) = (0, 0);
    unsafe {
        PowerReadACValueIndex(
            HKEY::default(),
            Some(scheme),
            Some(&GUID_SYSTEM_BUTTON_SUBGROUP),
            Some(&GUID_LIDCLOSE_ACTION),
            &mut ac,
        )
        .ok()
        .map_err(|e| format!("Could not read the lid setting: {e}"))?;
        if PowerReadDCValueIndex(
            HKEY::default(),
            Some(scheme),
            Some(&GUID_SYSTEM_BUTTON_SUBGROUP),
            Some(&GUID_LIDCLOSE_ACTION),
            &mut dc,
        ) != 0
        {
            return Err("Could not read the lid setting on battery.".into());
        }
    }
    Ok((ac, dc))
}

fn write(scheme: &GUID, ac: u32, dc: u32) -> Result<(), String> {
    unsafe {
        PowerWriteACValueIndex(
            HKEY::default(),
            scheme,
            Some(&GUID_SYSTEM_BUTTON_SUBGROUP),
            Some(&GUID_LIDCLOSE_ACTION),
            ac,
        )
        .ok()
        .map_err(|e| format!("Could not change the lid setting: {e}"))?;
        if PowerWriteDCValueIndex(
            HKEY::default(),
            scheme,
            Some(&GUID_SYSTEM_BUTTON_SUBGROUP),
            Some(&GUID_LIDCLOSE_ACTION),
            dc,
        ) != 0
        {
            return Err("Could not change the lid setting on battery.".into());
        }
        // Re-applying the plan makes the change take effect at once. Only the active one:
        // putting back a plan the user has since left must not switch back to it.
        if active_scheme()? != *scheme {
            return Ok(());
        }
        PowerSetActiveScheme(HKEY::default(), Some(scheme))
            .ok()
            .map_err(|e| format!("Could not apply the power plan: {e}"))
    }
}

pub struct Lid {
    record: PathBuf,
    held: Option<Saved>,
}

impl Lid {
    /// `record` holds the user's values while Doze has changed them. One left behind by a
    /// crash is restored here.
    pub fn new(record: PathBuf) -> Self {
        if let Some(saved) = std::fs::read(&record)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Saved>(&bytes).ok())
        {
            if write(&GUID::from_u128(saved.scheme), saved.ac, saved.dc).is_ok() {
                let _ = std::fs::remove_file(&record);
            }
        }
        Self { record, held: None }
    }

    /// Keeps the lid from sleeping the PC while `hold` is true; restores the user's setting
    /// otherwise.
    pub fn set(&mut self, hold: bool) -> Result<(), String> {
        match (hold, self.held) {
            (true, None) => {
                let scheme = active_scheme()?;
                let (ac, dc) = read(&scheme)?;
                let saved = Saved {
                    scheme: scheme.to_u128(),
                    ac,
                    dc,
                };
                if (ac, dc) != (DO_NOTHING, DO_NOTHING) {
                    std::fs::write(
                        &self.record,
                        serde_json::to_vec(&saved).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| format!("Could not save the lid setting: {e}"))?;
                    if let Err(error) = write(&scheme, DO_NOTHING, DO_NOTHING) {
                        let _ = std::fs::remove_file(&self.record);
                        return Err(error);
                    }
                }
                self.held = Some(saved);
                Ok(())
            }
            (false, Some(saved)) => {
                if (saved.ac, saved.dc) != (DO_NOTHING, DO_NOTHING) {
                    write(&GUID::from_u128(saved.scheme), saved.ac, saved.dc)?;
                    let _ = std::fs::remove_file(&self.record);
                }
                self.held = None;
                Ok(())
            }
            // The user switched power plans: give the old one its setting back and hold the
            // new one, which has its own lid action.
            (true, Some(saved))
                if active_scheme().is_ok_and(|active| active.to_u128() != saved.scheme) =>
            {
                self.set(false)?;
                self.set(true)
            }
            _ => Ok(()),
        }
    }

    /// For Settings › Advanced › Show active requests.
    pub fn describe(&self) -> Option<String> {
        self.held.map(|_| {
            "Lid close action: Do nothing (your setting returns when Doze lets go)".to_string()
        })
    }
}

impl Drop for Lid {
    fn drop(&mut self) {
        let _ = self.set(false);
    }
}

#[cfg(test)]
mod tests {
    /// Both tests change the same system-wide power plan, so they never run at once.
    static PLAN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    #[ignore = "Changes and restores the active power plan's lid setting"]
    fn holding_and_releasing_restores_the_lid_setting() {
        let _plan = PLAN.lock().unwrap_or_else(|e| e.into_inner());
        let scheme = super::active_scheme().unwrap();
        let before = super::read(&scheme).unwrap();
        let record = std::env::temp_dir().join(format!("doze-lid-{}.json", uuid::Uuid::new_v4()));
        let mut lid = super::Lid::new(record.clone());
        lid.set(true).unwrap();
        assert_eq!(super::read(&scheme).unwrap(), (0, 0));
        lid.set(false).unwrap();
        assert_eq!(super::read(&scheme).unwrap(), before);
        assert!(!record.exists());
    }

    #[test]
    #[ignore = "Switches the active power plan and its lid settings, then restores both"]
    fn switching_plans_while_held_moves_the_override_and_keeps_the_new_plan() {
        let _plan = PLAN.lock().unwrap_or_else(|e| e.into_inner());
        use windows::{core::GUID, Win32::System::Registry::HKEY};
        let activate = |scheme: &GUID| unsafe {
            windows::Win32::System::Power::PowerSetActiveScheme(HKEY::default(), Some(scheme))
                .ok()
                .unwrap()
        };
        let first = super::active_scheme().unwrap();
        // High performance or Power saver, whichever is not active.
        let other = [
            GUID::from_u128(0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c),
            GUID::from_u128(0xa1841308_3541_4fab_bc81_f71556f20b4a),
        ]
        .into_iter()
        .find(|scheme| *scheme != first)
        .unwrap();
        let (first_before, other_before) =
            (super::read(&first).unwrap(), super::read(&other).unwrap());
        let record = std::env::temp_dir().join(format!("doze-lid-{}.json", uuid::Uuid::new_v4()));
        let mut lid = super::Lid::new(record.clone());
        lid.set(true).unwrap();
        activate(&other);
        // The engine sets the lid again on every check while it holds the PC.
        lid.set(true).unwrap();
        let held = (super::read(&first).unwrap(), super::read(&other).unwrap());
        lid.set(false).unwrap();
        let released = (
            super::read(&other).unwrap(),
            super::active_scheme().unwrap(),
        );
        activate(&first);
        assert_eq!(held, (first_before, (0, 0)));
        assert_eq!(released, (other_before, other));
        assert!(!record.exists());
    }
}
