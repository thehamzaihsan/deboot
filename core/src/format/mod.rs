#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use crate::list::list_drives;
use crate::model::{FormatEvent, FormatRequest, Stage};

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn format_one<F: FnMut(FormatEvent)>(
    drive: &crate::model::UsbDrive,
    req: &FormatRequest,
    on_event: &mut F,
) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::format_drive(drive, req, on_event);
    #[cfg(target_os = "windows")]
    return windows::format_drive(drive, req, on_event);
}

/// Formats each requested drive, re-validating against a fresh device
/// listing first so nothing is ever wiped based on stale UI state.
///
/// On Linux all requests run inside a single elevated (`pkexec`) batch —
/// see `linux::format_batch` — so the GUI process itself never needs to run
/// as root. On Windows the whole app already runs elevated (requested via
/// the embedded manifest), and on macOS external-disk erases typically
/// don't need elevation at all, so those platforms format one drive at a
/// time.
pub fn format_drives<F: FnMut(FormatEvent)>(requests: &[FormatRequest], mut on_event: F) {
    let current = match list_drives() {
        Ok(drives) => drives,
        Err(e) => {
            for req in requests {
                on_event(FormatEvent {
                    drive_id: req.drive_id.clone(),
                    stage: Stage::Failed,
                    message: format!("could not re-check devices: {e}"),
                });
            }
            return;
        }
    };

    let mut pairs = Vec::new();
    for req in requests {
        match current.iter().find(|d| d.id == req.drive_id) {
            None => on_event(FormatEvent {
                drive_id: req.drive_id.clone(),
                stage: Stage::Failed,
                message: "drive is no longer connected".to_string(),
            }),
            Some(drive) if !drive.is_safe_to_format() => on_event(FormatEvent {
                drive_id: req.drive_id.clone(),
                stage: Stage::Failed,
                message: "refusing to format a non-removable or system disk".to_string(),
            }),
            Some(drive) => pairs.push((drive, req)),
        }
    }

    if pairs.is_empty() {
        return;
    }

    #[cfg(target_os = "linux")]
    linux::format_batch(&pairs, &mut on_event);

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    for (drive, req) in &pairs {
        if let Err(e) = format_one(drive, req, &mut on_event) {
            on_event(FormatEvent {
                drive_id: req.drive_id.clone(),
                stage: Stage::Failed,
                message: e.to_string(),
            });
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    for (drive, _) in &pairs {
        on_event(FormatEvent {
            drive_id: drive.id.clone(),
            stage: Stage::Failed,
            message: "unsupported platform".to_string(),
        });
    }
}
