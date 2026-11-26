use anyhow::Result;

#[cfg(windows)]
mod imp {
    use anyhow::{Context, Result};
    use std::sync::Mutex;
    use windows_sys::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
    };
    use winreg::enums::*;
    use winreg::RegKey;

    const INTERNET_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    const DEFAULT_BYPASS: &str = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;<local>";

    #[derive(Clone)]
    struct ProxyBackup {
        enable: u32,
        server: Option<String>,
        override_str: Option<String>,
    }

    static BACKUP: Mutex<Option<ProxyBackup>> = Mutex::new(None);

    fn notify_wininet() {
        unsafe {
            InternetSetOptionW(
                std::ptr::null_mut(),
                INTERNET_OPTION_SETTINGS_CHANGED,
                std::ptr::null(),
                0,
            );
            InternetSetOptionW(
                std::ptr::null_mut(),
                INTERNET_OPTION_REFRESH,
                std::ptr::null(),
                0,
            );
        }
    }

    pub fn enable_system_proxy(port: u16) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey(INTERNET_SETTINGS)
            .context("打开 Windows Internet Settings 注册表项失败")?;

        {
            let mut backup_guard = BACKUP.lock().unwrap();
            if backup_guard.is_none() {
                let enable: u32 = key.get_value("ProxyEnable").unwrap_or(0);
                let server: Option<String> = key.get_value("ProxyServer").ok();
                let override_str: Option<String> = key.get_value("ProxyOverride").ok();
                *backup_guard = Some(ProxyBackup {
                    enable,
                    server,
                    override_str,
                });
            }
        }

        key.set_value("ProxyEnable", &1u32)?;
        key.set_value("ProxyServer", &format!("127.0.0.1:{}", port))?;
        key.set_value("ProxyOverride", &DEFAULT_BYPASS)?;
        notify_wininet();
        Ok(())
    }

    pub fn disable_system_proxy() -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(INTERNET_SETTINGS)?;

        let backup = BACKUP.lock().unwrap().take();
        if let Some(prev) = backup {
            key.set_value("ProxyEnable", &prev.enable)?;
            if let Some(s) = prev.server {
                let _ = key.set_value("ProxyServer", &s);
            }
            if let Some(o) = prev.override_str {
                let _ = key.set_value("ProxyOverride", &o);
            }
        } else {
            key.set_value("ProxyEnable", &0u32)?;
        }
        notify_wininet();
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use anyhow::Result;
    pub fn enable_system_proxy(_port: u16) -> Result<()> {
        Ok(())
    }
    pub fn disable_system_proxy() -> Result<()> {
        Ok(())
    }
}

pub fn enable_system_proxy(port: u16) -> Result<()> {
    imp::enable_system_proxy(port)
}

pub fn disable_system_proxy() -> Result<()> {
    imp::disable_system_proxy()
}
