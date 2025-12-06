use crate::imp::prelude::*;
use std::{env, fs, fs::OpenOptions, io};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use zip::{result::ZipError, ZipArchive};

#[derive(Debug, Clone, PartialEq)]
pub struct Driver {
    path: PathBuf
}

impl Driver {
    const ZIP: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), env!("SEP"), "driver.zip"));
    const PLATFORM: &'static str = include_str!(concat!(env!("OUT_DIR"), env!("SEP"), "platform"));

    pub fn install() -> io::Result<Self> {
        let this = Self::new(Self::default_dest());
        if !this.path.is_dir() {
            this.prepare()?;
        }
        this.ensure_launcher_stub()?;
        Ok(this)
    }

    /// Without prepare
    pub fn new<P: Into<PathBuf>>(path: P) -> Self { Self { path: path.into() } }
    ///
    pub fn prepare(&self) -> Result<(), ZipError> {
        fs::create_dir_all(&self.path)?;
        let mut a = ZipArchive::new(io::Cursor::new(Self::ZIP))?;
        a.extract(&self.path)
    }

    pub fn default_dest() -> PathBuf {
        if let Ok(dir) = env::var("PLAYWRIGHT_DRIVER_DIR") {
            return PathBuf::from(dir);
        }

        let base: PathBuf = dirs::cache_dir().unwrap_or_else(env::temp_dir);
        let dir: PathBuf = [
            base.as_os_str(),
            "ms-playwright".as_ref(),
            "playwright-rust".as_ref(),
            "driver".as_ref()
        ]
        .iter()
        .collect();

        // Ensure directory exists and is writable; if not, fall back to a temp location that is
        // typically permitted inside sandboxes.
        if dir.metadata().is_err() {
            if fs::create_dir_all(&dir).is_err() {
                return env::temp_dir().join("playwright-rust").join("driver");
            }
        }

        let probe = dir.join(".write_test");
        match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&probe)
        {
            Ok(_) => {
                let _ = fs::remove_file(&probe);
                dir
            }
            Err(_) => env::temp_dir().join("playwright-rust").join("driver")
        }
    }

    pub fn platform(&self) -> Platform {
        match Self::PLATFORM {
            "linux" => Platform::Linux,
            "linux-arm64" => Platform::LinuxArm64,
            "mac" => Platform::Mac,
            "mac-arm64" => Platform::MacArm64,
            "win32" => Platform::Win32,
            "win32_x64" => Platform::Win32x64,
            _ => unreachable!()
        }
    }

    pub fn executable(&self) -> PathBuf { self.launcher_path() }

    fn launcher_path(&self) -> PathBuf {
        match self.platform() {
            Platform::Linux | Platform::LinuxArm64 | Platform::Mac | Platform::MacArm64 => {
                self.path.join("playwright.sh")
            }
            Platform::Win32 | Platform::Win32x64 => self.path.join("playwright.cmd")
        }
    }

    fn ensure_launcher_stub(&self) -> io::Result<()> {
        let launcher = self.launcher_path();

        #[cfg(windows)]
        let stub = {
            let node = {
                let exe = self.path.join("node.exe");
                if exe.exists() {
                    exe
                } else {
                    self.path.join("node")
                }
            };
            // Keep the driver process alive when invoked with `run-driver`
            // by delegating to a tiny inline script that starts the driver
            // and parks the event loop.
            format!(
                "@echo off\r\nsetlocal\r\nset \"DIR=%~dp0\"\r\nset \"PW_DRIVER_DIR=%DIR%\"\r\nif \"%1\"==\"run-driver\" (\r\n  \"{node}\" -e \"const path=require('path');const dir=process.env.PW_DRIVER_DIR;const driver=require(path.join(dir,'package','lib','cli','driver'));driver.runDriver();setInterval(()=>{{}},2147483647);\" %*\r\n) else (\r\n  \"{node}\" \"%DIR%package\\cli.js\" %*\r\n)\r\n",
                node = node
                    .file_name()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_else(|| "node".into())
            )
        };

        #[cfg(not(windows))]
        let stub = r#"#!/bin/sh
set -e
DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
export PW_DRIVER_DIR="$DIR"
if [ "${1-}" = "run-driver" ]; then
  exec "$DIR/node" -e "const path=require('path');const dir=process.env.PW_DRIVER_DIR;const driver=require(path.join(dir,'package','lib','cli','driver'));driver.runDriver();setInterval(()=>{},2147483647);" "$@"
else
  exec "$DIR/node" "$DIR/package/cli.js" "$@"
fi
"#;

        fs::write(&launcher, stub)?;

        #[cfg(unix)]
        {
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&launcher, perms)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Linux,
    LinuxArm64,
    Win32,
    Win32x64,
    Mac,
    MacArm64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install() { let _driver = Driver::install().unwrap(); }
}
