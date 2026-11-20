use std::process::Command;

use anyhow::{anyhow, Result};

use crate::model::{FormatEvent, FormatRequest, Stage, UsbDrive};

fn run_diskpart_script(lines: &[String]) -> Result<()> {
    let path = std::env::temp_dir().join(format!("deboot_{}.txt", std::process::id()));
    std::fs::write(&path, lines.join("\r\n"))
        .map_err(|e| anyhow!("failed to write diskpart script: {e}"))?;

    let output = Command::new("diskpart")
        .args(["/s", path.to_string_lossy().as_ref()])
        .output()
        .map_err(|e| anyhow!("failed to run diskpart: {e}"))?;

    let _ = std::fs::remove_file(&path);

    if !output.status.success() {
        return Err(anyhow!(
            "diskpart failed: {}",
            String::from_utf8_lossy(&output.stdout).trim()
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

    emit(on_event, Stage::Unmounting, "Preparing disk");
    emit(on_event, Stage::Wiping, "Cleaning disk");
    run_diskpart_script(&[
        format!("select disk {}", drive.id),
        "clean".to_string(),
    ])?;

    emit(on_event, Stage::Partitioning, "Creating new partition");
    run_diskpart_script(&[
        format!("select disk {}", drive.id),
        "create partition primary".to_string(),
    ])?;

    emit(on_event, Stage::Formatting, "Writing FAT32 filesystem");
    let label = req.volume_label.chars().take(11).collect::<String>();
    run_diskpart_script(&[
        format!("select disk {}", drive.id),
        "select partition 1".to_string(),
        format!("format fs=fat32 label=\"{label}\" quick"),
        "assign".to_string(),
    ])?;

    emit(on_event, Stage::Done, "Formatted successfully");
    Ok(())
}
