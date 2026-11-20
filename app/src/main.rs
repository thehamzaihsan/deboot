use deboot_core::{FilesystemKind, FormatEvent, FormatRequest, Stage, UsbDrive};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

slint::include_modules!();

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size:.0} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

fn to_item(drive: &UsbDrive) -> DriveItem {
    DriveItem {
        id: drive.id.clone().into(),
        name: drive.display_name.clone().into(),
        size_text: human_size(drive.size_bytes).into(),
        mount_text: if drive.mount_points.is_empty() {
            "not mounted".into()
        } else {
            drive.mount_points.join(", ").into()
        },
        selected: false,
        stage_text: Default::default(),
        is_busy: false,
        is_done: false,
        is_failed: false,
        error_text: Default::default(),
    }
}

fn selected_count(drives: &ModelRc<DriveItem>) -> i32 {
    (0..drives.row_count())
        .filter(|&i| drives.row_data(i).map(|d| d.selected).unwrap_or(false))
        .count() as i32
}

fn refresh(window: &AppWindow) {
    let items = match deboot_core::list_drives() {
        Ok(drives) => drives.iter().map(to_item).collect::<Vec<_>>(),
        Err(e) => {
            eprintln!("failed to list drives: {e}");
            Vec::new()
        }
    };
    window.set_drives(ModelRc::new(VecModel::from(items)));
    window.set_selected_count(0);
}

fn main() -> anyhow::Result<()> {
    let window = AppWindow::new()?;
    refresh(&window);

    {
        let window_weak = window.as_weak();
        window.on_refresh_requested(move || {
            if let Some(window) = window_weak.upgrade() {
                refresh(&window);
            }
        });
    }

    {
        let window_weak = window.as_weak();
        window.on_drive_toggled(move |id| {
            let Some(window) = window_weak.upgrade() else { return };
            let drives = window.get_drives();
            for i in 0..drives.row_count() {
                if let Some(mut item) = drives.row_data(i) {
                    if item.id == id {
                        item.selected = !item.selected;
                        drives.set_row_data(i, item);
                        break;
                    }
                }
            }
            window.set_selected_count(selected_count(&drives));
        });
    }

    {
        let window_weak = window.as_weak();
        window.on_format_requested(move || {
            let Some(window) = window_weak.upgrade() else { return };

            let drives = window.get_drives();
            let label = window.get_volume_label().to_string();
            let mut requests = Vec::new();

            for i in 0..drives.row_count() {
                if let Some(mut item) = drives.row_data(i) {
                    if item.selected {
                        requests.push(FormatRequest {
                            drive_id: item.id.to_string(),
                            filesystem: FilesystemKind::Fat32,
                            volume_label: label.clone(),
                        });
                        item.is_busy = true;
                        item.is_done = false;
                        item.is_failed = false;
                        item.stage_text = "Starting…".into();
                        drives.set_row_data(i, item);
                    }
                }
            }

            if requests.is_empty() {
                return;
            }

            window.set_job_running(true);
            let window_weak = window.as_weak();

            std::thread::spawn(move || {
                let event_window_weak = window_weak.clone();
                deboot_core::format_drives(&requests, move |event: FormatEvent| {
                    let window_weak = event_window_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        apply_event(window_weak, event);
                    });
                });

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = window_weak.upgrade() {
                        window.set_job_running(false);
                    }
                });
            });
        });
    }

    window.run()?;
    Ok(())
}

fn apply_event(window_weak: slint::Weak<AppWindow>, event: FormatEvent) {
    let Some(window) = window_weak.upgrade() else { return };
    let drives = window.get_drives();
    for i in 0..drives.row_count() {
        if let Some(mut item) = drives.row_data(i) {
            if item.id.as_str() == event.drive_id {
                match event.stage {
                    Stage::Done => {
                        item.is_busy = false;
                        item.is_done = true;
                        item.stage_text = event.message.into();
                    }
                    Stage::Failed => {
                        item.is_busy = false;
                        item.is_failed = true;
                        item.error_text = event.message.into();
                    }
                    other => {
                        item.is_busy = true;
                        item.stage_text = other.text().into();
                    }
                }
                drives.set_row_data(i, item);
                break;
            }
        }
    }
}
