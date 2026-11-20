use std::process::Command;

use anyhow::{anyhow, Result};

use crate::model::{FormatEvent, FormatRequest, Stage, UsbDrive};

fn run(args: &[&str]) -> Result<String> {
    let output = Command::new("diskutil")
        .args(args)
        .output()
        .map_err(|e| anyhow!("failed to run diskutil: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "diskutil {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_elevated(shell_cmd: &str) -> Result<()> {
    let script = format!("do shell script \"{shell_cmd}\" with administrator privileges");
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| anyhow!("failed to run osascript: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "elevated diskutil failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

pub fn format_drive<F: FnMut(FormatEvent)>(
    drive: &UsbDrive,
    req: &FormatRequest,
    on_event: &mut F,
) -> Result<()> {
    let emit = |on_event: &mut F, stage: Stage, message: &str| {
        on_event(FormatEvent {
            drive_id: drive.id.clone(),
            stage,
            message: message.to_string(),
        });
    };

    let device = drive.id.trim_start_matches("/dev/");
    let label = req.volume_label.chars().take(11).collect::<String>();

    emit(on_event, Stage::Unmounting, "Unmounting disk");
    emit(on_event, Stage::Wiping, "Erasing disk");
    emit(on_event, Stage::Partitioning, "Creating new partition");
    emit(on_event, Stage::Formatting, "Writing FAT32 filesystem");

    let direct = run(&["eraseDisk", "FAT32", &label, device]);
    if let Err(e) = direct {
        run_elevated(&format!("diskutil eraseDisk FAT32 {label} {device}"))
            .map_err(|_| e)?;
    }

    emit(on_event, Stage::Done, "Formatted successfully");
    Ok(())
}
