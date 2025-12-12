use std::ffi::{CStr, CString};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use log::{debug, warn};
use walkdir::WalkDir;

const DEV_PATH: &str = "/dev/hymo_ctl";
const HYMO_IOC_MAGIC: u8 = 0xE0;
const EXPECTED_PROTOCOL_VERSION: i32 = 4;

const _IOC_NRBITS: u32 = 8;
const _IOC_TYPEBITS: u32 = 8;
const _IOC_SIZEBITS: u32 = 14;
const _IOC_DIRBITS: u32 = 2;

const _IOC_NRSHIFT: u32 = 0;
const _IOC_TYPESHIFT: u32 = _IOC_NRSHIFT + _IOC_NRBITS;
const _IOC_SIZESHIFT: u32 = _IOC_TYPESHIFT + _IOC_TYPEBITS;
const _IOC_DIRSHIFT: u32 = _IOC_SIZESHIFT + _IOC_SIZEBITS;

const _IOC_NONE: u32 = 0;
const _IOC_WRITE: u32 = 1;
const _IOC_READ: u32 = 2;
const _IOC_READ_WRITE: u32 = 3;

macro_rules! _IOC {
    ($dir:expr, $type:expr, $nr:expr, $size:expr) => {
        (($dir) << _IOC_DIRSHIFT) |
        (($type) << _IOC_TYPESHIFT) |
        (($nr) << _IOC_NRSHIFT) |
        (($size) << _IOC_SIZESHIFT)
    };
}

macro_rules! _IO {
    ($type:expr, $nr:expr) => {
        _IOC!(_IOC_NONE, $type, $nr, 0)
    };
}

macro_rules! _IOR {
    ($type:expr, $nr:expr, $size:ty) => {
        _IOC!(_IOC_READ, $type, $nr, std::mem::size_of::<$size>() as u32)
    };
}

macro_rules! _IOW {
    ($type:expr, $nr:expr, $size:ty) => {
        _IOC!(_IOC_WRITE, $type, $nr, std::mem::size_of::<$size>() as u32)
    };
}

macro_rules! _IOWR {
    ($type:expr, $nr:expr, $size:ty) => {
        _IOC!(_IOC_READ_WRITE, $type, $nr, std::mem::size_of::<$size>() as u32)
    };
}

#[repr(C)]
struct HymoIoctlArg {
    src: *const libc::c_char,
    target: *const libc::c_char,
    r#type: u8,
}

#[repr(C)]
struct HymoIoctlListArg {
    buf: *mut libc::c_char,
    size: usize,
}

fn ioc_add_rule() -> libc::c_int { _IOW!(HYMO_IOC_MAGIC as u32, 1, HymoIoctlArg) as libc::c_int }
fn ioc_del_rule() -> libc::c_int { _IOW!(HYMO_IOC_MAGIC as u32, 2, HymoIoctlArg) as libc::c_int }
fn ioc_hide_rule() -> libc::c_int { _IOW!(HYMO_IOC_MAGIC as u32, 3, HymoIoctlArg) as libc::c_int }
fn ioc_inject_rule() -> libc::c_int { _IOW!(HYMO_IOC_MAGIC as u32, 4, HymoIoctlArg) as libc::c_int }
fn ioc_clear_all() -> libc::c_int { _IO!(HYMO_IOC_MAGIC as u32, 5) as libc::c_int }
fn ioc_get_version() -> libc::c_int { _IOR!(HYMO_IOC_MAGIC as u32, 6, libc::c_int) as libc::c_int }
fn ioc_list_rules() -> libc::c_int { _IOWR!(HYMO_IOC_MAGIC as u32, 7, HymoIoctlListArg) as libc::c_int }

#[derive(Debug, PartialEq)]
pub enum HymoFsStatus {
    Available,
    NotPresent,
    KernelTooOld,
    ModuleTooOld,
}

pub struct HymoFs;

impl HymoFs {
    fn open_dev() -> Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(DEV_PATH)
            .with_context(|| format!("Failed to open {}", DEV_PATH))
    }

    pub fn check_status() -> HymoFsStatus {
        if Path::new(DEV_PATH).exists() {
            HymoFsStatus::Available
        } else {
            HymoFsStatus::NotPresent
        }
    }

    pub fn is_available() -> bool {
        Self::check_status() == HymoFsStatus::Available
    }

    pub fn get_version() -> Option<i32> {
        let file = Self::open_dev().ok()?;
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), ioc_get_version())
        };
        if ret < 0 {
            None
        } else {
            Some(ret)
        }
    }

    pub fn clear() -> Result<()> {
        let file = Self::open_dev()?;
        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), ioc_clear_all())
        };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS clear failed: {}", err);
        }
        Ok(())
    }

    pub fn add_rule(src: &str, target: &str, type_val: i32) -> Result<()> {
        let file = Self::open_dev()?;
        let c_src = CString::new(src)?;
        let c_target = CString::new(target)?;
        
        let arg = HymoIoctlArg {
            src: c_src.as_ptr(),
            target: c_target.as_ptr(),
            r#type: type_val as u8,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), ioc_add_rule(), &arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS add_rule failed: {}", err);
        }
        Ok(())
    }

    pub fn delete_rule(src: &str) -> Result<()> {
        let file = Self::open_dev()?;
        let c_src = CString::new(src)?;
        
        let arg = HymoIoctlArg {
            src: c_src.as_ptr(),
            target: std::ptr::null(),
            r#type: 0,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), ioc_del_rule(), &arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS delete_rule failed: {}", err);
        }
        Ok(())
    }

    pub fn hide_path(path: &str) -> Result<()> {
        let file = Self::open_dev()?;
        let c_path = CString::new(path)?;
        
        let arg = HymoIoctlArg {
            src: c_path.as_ptr(),
            target: std::ptr::null(),
            r#type: 0,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), ioc_hide_rule(), &arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS hide_path failed: {}", err);
        }
        Ok(())
    }

    pub fn inject_dir(dir: &str) -> Result<()> {
        let file = Self::open_dev()?;
        let c_dir = CString::new(dir)?;
        
        let arg = HymoIoctlArg {
            src: c_dir.as_ptr(),
            target: std::ptr::null(),
            r#type: 0,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), ioc_inject_rule(), &arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS inject_dir failed: {}", err);
        }
        Ok(())
    }

    pub fn list_active_rules() -> Result<String> {
        let file = Self::open_dev()?;
        let capacity = 128 * 1024;
        let mut buffer = vec![0u8; capacity];
        let mut arg = HymoIoctlListArg {
            buf: buffer.as_mut_ptr() as *mut libc::c_char,
            size: capacity,
        };

        let ret = unsafe {
            libc::ioctl(file.as_raw_fd(), ioc_list_rules(), &mut arg)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("HymoFS list_rules failed: {}", err);
        }

        let c_str = unsafe { CStr::from_ptr(buffer.as_ptr() as *const libc::c_char) };
        Ok(c_str.to_string_lossy().into_owned())
    }

    pub fn inject_directory(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("HymoFS walk error: {}", e);
                    continue;
                }
            };

            let current_path = entry.path();
            let relative_path = match current_path.strip_prefix(module_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let target_path = target_base.join(relative_path);
            let file_type = entry.file_type();

            if file_type.is_file() || file_type.is_symlink() {
                if let Err(e) = Self::add_rule(
                    &target_path.to_string_lossy(),
                    &current_path.to_string_lossy(),
                    0 
                ) {
                    warn!("Failed to add rule for {}: {}", target_path.display(), e);
                }
            } else if file_type.is_char_device() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.rdev() == 0 {
                        if let Err(e) = Self::hide_path(&target_path.to_string_lossy()) {
                            warn!("Failed to hide path {}: {}", target_path.display(), e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn delete_directory_rules(target_base: &Path, module_dir: &Path) -> Result<()> {
        if !module_dir.exists() || !module_dir.is_dir() {
            return Ok(());
        }

        for entry in WalkDir::new(module_dir).min_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("HymoFS walk error: {}", e);
                    continue;
                }
            };

            let current_path = entry.path();
            let relative_path = match current_path.strip_prefix(module_dir) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let target_path = target_base.join(relative_path);
            let file_type = entry.file_type();

            if file_type.is_file() || file_type.is_symlink() {
                if let Err(e) = Self::delete_rule(&target_path.to_string_lossy()) {
                    warn!("Failed to delete rule for {}: {}", target_path.display(), e);
                }
            } else if file_type.is_char_device() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.rdev() == 0 {
                        if let Err(e) = Self::delete_rule(&target_path.to_string_lossy()) {
                            warn!("Failed to delete hidden rule for {}: {}", target_path.display(), e);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
