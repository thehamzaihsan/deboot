use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::model::UsbDrive;

const CRITICAL_MOUNTS: &[&str] = &[
    "/", "/boot", "/boot/efi", "/efi", "/home", "/var", "/usr", "/opt", "[SWAP]",
];

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<BlockDevice>,
}

#[derive(Debug, Deserialize)]
struct BlockDevice {
    name: String,
    path: Option<String>,
    #[serde(default)]
    size: u64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    rm: bool,
    #[serde(default)]
    vendor: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    mountpoints: Vec<Option<String>>,
    #[serde(default)]
    children: Vec<BlockDevice>,
}

fn collect_mountpoints(dev: &BlockDevice, out: &mut Vec<String>) {
    out.extend(dev.mountpoints.iter().flatten().cloned());
    for child in &dev.children {
        collect_mountpoints(child, out);
    }
}

pub fn list_drives() -> Result<Vec<UsbDrive>> {
    let output = Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,PATH,SIZE,TYPE,RM,TRAN,VENDOR,MODEL,MOUNTPOINTS",
        ])
        .output()
        .context("failed to run lsblk")?;

    if !output.status.success() {
        anyhow::bail!(
            "lsblk exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let parsed: LsblkOutput =
        serde_json::from_slice(&output.stdout).context("failed to parse lsblk JSON")?;

    let mut drives = Vec::new();
    for dev in &parsed.blockdevices {
        if dev.kind != "disk" || !dev.rm {
            continue;
        }

        let mut mounts = Vec::new();
        collect_mountpoints(dev, &mut mounts);
        let is_system_disk = mounts.iter().any(|m| CRITICAL_MOUNTS.contains(&m.as_str()));

        let vendor = dev.vendor.as_deref().unwrap_or("").trim();
        let model = dev.model.as_deref().unwrap_or("").trim();
        let display_name = match (vendor.is_empty(), model.is_empty()) {
            (true, true) => dev.name.clone(),
            (true, false) => model.to_string(),
            (false, true) => vendor.to_string(),
            (false, false) => format!("{vendor} {model}"),
        };

        drives.push(UsbDrive {
            id: dev.path.clone().unwrap_or_else(|| format!("/dev/{}", dev.name)),
            display_name,
            size_bytes: dev.size,
            is_removable: dev.rm,
            is_system_disk,
            mount_points: mounts,
        });
    }

    Ok(drives)
}
