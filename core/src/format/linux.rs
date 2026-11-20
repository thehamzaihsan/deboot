use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::model::{FormatEvent, FormatRequest, Stage, UsbDrive};

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn fat_label(label: &str) -> String {
    label.to_uppercase().chars().take(11).collect()
}

const SCRIPT_PRELUDE: &str = r#"
emit() { echo "DEBOOT_STAGE|$1|$2|$3"; }
err() { echo "DEBOOT_ERROR|$1|$2"; }

format_one() {
  local device="$1"; local label="$2"; shift 2
  local id="$device"
  local out part

  emit "$id" Unmounting "Unmounting partitions"
  for mp in "$@"; do umount "$mp" >/dev/null 2>&1; done

  emit "$id" Wiping "Wiping existing signatures"
  if ! out=$(wipefs -a "$device" 2>&1); then err "$id" "wipefs failed: $out"; return; fi

  emit "$id" Partitioning "Creating new partition table"
  if ! out=$(parted -s "$device" mklabel msdos mkpart primary fat32 1MiB 100% 2>&1); then
    err "$id" "parted failed: $out"; return
  fi
  partprobe "$device" >/dev/null 2>&1 || true

  if [[ "$device" =~ [0-9]$ ]]; then part="${device}p1"; else part="${device}1"; fi
  for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
    [[ -e "$part" ]] && break
    sleep 0.2
  done
  if [[ ! -e "$part" ]]; then err "$id" "kernel never exposed new partition $part"; return; fi

  emit "$id" Formatting "Writing FAT32 filesystem"
  if ! out=$(mkfs.vfat -F 32 -n "$label" "$part" 2>&1); then err "$id" "mkfs.vfat failed: $out"; return; fi

  emit "$id" Done "Formatted successfully"
}

"#;

fn build_script(pairs: &[(&UsbDrive, &FormatRequest)]) -> String {
    let mut script = String::from("#!/usr/bin/env bash\nset -u\n");
    script.push_str(SCRIPT_PRELUDE);

    for (drive, req) in pairs {
        let label = fat_label(&req.volume_label);
        script.push_str("format_one ");
        script.push_str(&sh_quote(&drive.id));
        script.push(' ');
        script.push_str(&sh_quote(&label));
        for mp in &drive.mount_points {
            if mp.starts_with('[') {
                continue; // e.g. "[SWAP]" has no real path to unmount
            }
            script.push(' ');
            script.push_str(&sh_quote(mp));
        }
        script.push('\n');
    }
    script
}

/// Runs every requested drive's wipe+format sequence inside a single
/// `pkexec` invocation (one authorization prompt for the whole batch,
/// rather than one per privileged command or one per drive), streaming
/// per-drive stage markers back out as they happen. The GUI process itself
/// never runs as root, which is required for it to keep working under
/// Wayland (a root process generally cannot attach to another user's
/// Wayland session).
pub fn format_batch<F: FnMut(FormatEvent)>(pairs: &[(&UsbDrive, &FormatRequest)], on_event: &mut F) {
    let script = build_script(pairs);
    let script_path = std::env::temp_dir().join(format!("deboot_{}.sh", std::process::id()));
    if let Err(e) = std::fs::write(&script_path, &script) {
        for (drive, _) in pairs {
            on_event(FormatEvent {
                drive_id: drive.id.clone(),
                stage: Stage::Failed,
                message: format!("failed to write helper script: {e}"),
            });
        }
        return;
    }

    let mut pending: std::collections::HashSet<String> =
        pairs.iter().map(|(d, _)| d.id.clone()).collect();

    let child = Command::new("pkexec")
        .arg("bash")
        .arg(&script_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    match child {
        Ok(mut child) => {
            if let Some(stdout) = child.stdout.take() {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some(rest) = line.strip_prefix("DEBOOT_STAGE|") {
                        let mut parts = rest.splitn(3, '|');
                        if let (Some(id), Some(stage), Some(message)) =
                            (parts.next(), parts.next(), parts.next())
                        {
                            let stage = match stage {
                                "Unmounting" => Stage::Unmounting,
                                "Wiping" => Stage::Wiping,
                                "Partitioning" => Stage::Partitioning,
                                "Formatting" => Stage::Formatting,
                                "Done" => {
                                    pending.remove(id);
                                    Stage::Done
                                }
                                _ => continue,
                            };
                            on_event(FormatEvent {
                                drive_id: id.to_string(),
                                stage,
                                message: message.to_string(),
                            });
                        }
                    } else if let Some(rest) = line.strip_prefix("DEBOOT_ERROR|") {
                        let mut parts = rest.splitn(2, '|');
                        if let (Some(id), Some(message)) = (parts.next(), parts.next()) {
                            pending.remove(id);
                            on_event(FormatEvent {
                                drive_id: id.to_string(),
                                stage: Stage::Failed,
                                message: message.to_string(),
                            });
                        }
                    }
                }
            }
            let _ = child.wait();
        }
        Err(e) => {
            on_event(FormatEvent {
                drive_id: String::new(),
                stage: Stage::Failed,
                message: format!("failed to launch pkexec: {e}"),
            });
        }
    }

    let _ = std::fs::remove_file(&script_path);

    for id in pending {
        on_event(FormatEvent {
            drive_id: id,
            stage: Stage::Failed,
            message: "cancelled or authorization failed".to_string(),
        });
    }
}
