use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::model::UsbDrive;

#[derive(Debug, Deserialize)]
struct WinDisk {
    #[serde(rename = "Number")]
    number: i64,
    #[serde(rename = "FriendlyName")]
    friendly_name: Option<String>,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "IsSystem")]
    is_system: bool,
    #[serde(rename = "IsBoot")]
    is_boot: bool,
    #[serde(rename = "MountPoints")]
    #[serde(default)]
    mount_points: Vec<String>,
}

// PowerShell's ConvertTo-Json collapses a single-element array to a bare
// object instead of a one-element JSON array, so callers must accept both.
fn parse_disks(json: &[u8]) -> Result<Vec<WinDisk>> {
    if let Ok(list) = serde_json::from_slice::<Vec<WinDisk>>(json) {
        return Ok(list);
    }
    let single: WinDisk =
        serde_json::from_slice(json).context("failed to parse Get-Disk JSON")?;
    Ok(vec![single])
}

pub fn list_drives() -> Result<Vec<UsbDrive>> {
    let script = r#"
$disks = Get-Disk | Where-Object { $_.BusType -eq 'USB' } | ForEach-Object {
    $num = $_.Number
    $mounts = @(Get-Partition -DiskNumber $num -ErrorAction SilentlyContinue |
        Where-Object { $_.DriveLetter } |
        ForEach-Object { "$($_.DriveLetter):" })
    [PSCustomObject]@{
        Number = $num
        FriendlyName = $_.FriendlyName
        Size = $_.Size
        IsSystem = [bool]$_.IsSystem
        IsBoot = [bool]$_.IsBoot
        MountPoints = $mounts
    }
}
$disks | ConvertTo-Json -Compress -Depth 4
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .context("failed to run powershell Get-Disk")?;

    if !output.status.success() {
        anyhow::bail!(
            "Get-Disk exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let trimmed = output.stdout.as_slice();
    if trimmed.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(Vec::new());
    }

    let disks = parse_disks(trimmed)?;

    Ok(disks
        .into_iter()
        .map(|d| UsbDrive {
            id: d.number.to_string(),
            display_name: d
                .friendly_name
                .unwrap_or_else(|| format!("USB Disk {}", d.number)),
            size_bytes: d.size,
            is_removable: true,
            is_system_disk: d.is_system || d.is_boot,
            mount_points: d.mount_points,
        })
        .collect())
}
