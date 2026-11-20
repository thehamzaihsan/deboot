use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsbDrive {
    pub id: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub is_removable: bool,
    pub is_system_disk: bool,
    pub mount_points: Vec<String>,
}

impl UsbDrive {
    pub fn is_safe_to_format(&self) -> bool {
        self.is_removable && !self.is_system_disk
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemKind {
    Fat32,
}

impl FilesystemKind {
    pub fn label(&self) -> &'static str {
        match self {
            FilesystemKind::Fat32 => "FAT32",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormatRequest {
    pub drive_id: String,
    pub filesystem: FilesystemKind,
    pub volume_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Unmounting,
    Wiping,
    Partitioning,
    Formatting,
    Done,
    Failed,
}

impl Stage {
    pub fn text(&self) -> &'static str {
        match self {
            Stage::Unmounting => "Unmounting",
            Stage::Wiping => "Wiping",
            Stage::Partitioning => "Partitioning",
            Stage::Formatting => "Formatting",
            Stage::Done => "Done",
            Stage::Failed => "Failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormatEvent {
    pub drive_id: String,
    pub stage: Stage,
    pub message: String,
}
