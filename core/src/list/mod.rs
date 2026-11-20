#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use anyhow::Result;

use crate::model::UsbDrive;

pub fn list_drives() -> Result<Vec<UsbDrive>> {
    #[cfg(target_os = "linux")]
    return linux::list_drives();

    #[cfg(target_os = "macos")]
    return macos::list_drives();

    #[cfg(target_os = "windows")]
    return windows::list_drives();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("unsupported platform")
    }
}
