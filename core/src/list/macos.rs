use std::process::Command;

use anyhow::{Context, Result};
use plist::Value;

use crate::model::UsbDrive;

fn dict_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_dictionary()?.get(key)
}

fn get_str(value: &Value, key: &str) -> Option<String> {
    dict_get(value, key)?.as_string().map(|s| s.to_string())
}

fn get_u64(value: &Value, key: &str) -> Option<u64> {
    dict_get(value, key)?.as_unsigned_integer()
}

fn get_bool(value: &Value, key: &str, default: bool) -> bool {
    dict_get(value, key).and_then(Value::as_boolean).unwrap_or(default)
}

fn run_plist(args: &[&str]) -> Result<Value> {
    let output = Command::new("diskutil")
        .args(args)
        .output()
        .with_context(|| format!("failed to run diskutil {args:?}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "diskutil {:?} exited with {}: {}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Value::from_reader(std::io::Cursor::new(output.stdout))
        .context("failed to parse diskutil plist output")
}

fn whole_disk_mount_points(disk: &Value) -> Vec<String> {
    let mut mounts = Vec::new();
    if let Some(partitions) = dict_get(disk, "Partitions").and_then(Value::as_array) {
        for partition in partitions {
            if let Some(mp) = get_str(partition, "MountPoint") {
                mounts.push(mp);
            }
        }
    }
    mounts
}

fn is_critical_mount(mp: &str) -> bool {
    mp == "/" || mp.starts_with("/System")
}

pub fn list_drives() -> Result<Vec<UsbDrive>> {
    let listing = run_plist(&["list", "-plist", "external", "physical"])?;

    let disks = match dict_get(&listing, "AllDisksAndPartitions").and_then(Value::as_array) {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let mut drives = Vec::new();
    for disk in disks {
        let Some(device_id) = get_str(disk, "DeviceIdentifier") else {
            continue;
        };
        let size_bytes = get_u64(disk, "Size").unwrap_or(0);
        let mount_points = whole_disk_mount_points(disk);
        let is_system_disk = mount_points.iter().any(|m| is_critical_mount(m));

        let info = run_plist(&["info", "-plist", &device_id]).ok();
        let display_name = info
            .as_ref()
            .and_then(|i| get_str(i, "MediaName"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| device_id.clone());
        let is_removable = info
            .as_ref()
            .map(|i| !get_bool(i, "Internal", false))
            .unwrap_or(true);

        drives.push(UsbDrive {
            id: format!("/dev/{device_id}"),
            display_name,
            size_bytes,
            is_removable,
            is_system_disk,
            mount_points,
        });
    }

    Ok(drives.into_iter().filter(|d| d.is_removable).collect())
}
