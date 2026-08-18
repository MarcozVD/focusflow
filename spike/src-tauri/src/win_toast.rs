//! Identidad de las notificaciones de Windows.
//!
//! `tauri-plugin-notification` envía los toasts con el AppUserModelID
//! `com.focusflow.spike` (el `identifier` del bundle), pero Windows solo
//! muestra nombre e icono propios si existe un acceso directo en el menú
//! Inicio con ESE AppUserModelID. Sin ese registro, el sistema degrada el
//! toast y lo muestra como si viniera de PowerShell.
//!
//! Al arrancar (solo en la app instalada) este módulo:
//! 1. Fija el AppUserModelID del proceso actual.
//! 2. Crea/actualiza `FocusFlow.lnk` en el menú Inicio apuntando al exe,
//!    con la propiedad `System.AppUserModel.ID` = `com.focusflow.spike`.

#![cfg(windows)]

use windows::core::{HSTRING, Interface, PWSTR, PCWSTR};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, IPersistFile,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{IShellLinkW, SetCurrentProcessExplicitAppUserModelID, ShellLink};

/// Debe coincidir con `identifier` en tauri.conf.json.
pub const AUMID: &str = "com.focusflow.spike";

fn w(s: &str) -> HSTRING {
    HSTRING::from(s)
}


/// PROPVARIANT VT_LPWSTR construido a mano (el crate no expone
/// `InitPropVariantFromString` escalar). `SetValue` copia la cadena, así que
/// el buffer `wide` solo tiene que vivir hasta que vuelve la llamada.
unsafe fn propvariant_str(wide: &[u16]) -> PROPVARIANT {
    let mut pv = PROPVARIANT::default();
    pv.Anonymous.Anonymous = core::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
        vt: VT_LPWSTR,
        wReserved1: 0,
        wReserved2: 0,
        wReserved3: 0,
        Anonymous: PROPVARIANT_0_0_0 {
            pwszVal: PWSTR(wide.as_ptr() as *mut u16),
        },
    });
    pv
}

/// Idempotente y barato: se puede llamar en cada arranque. En builds de
/// desarrollo (target\debug / target\release) solo fija el AUMID del proceso
/// y no toca el menú Inicio.
pub fn ensure_toast_identity() -> Result<(), String> {
    unsafe {
        let aumid = w(AUMID);
        // 1. AUMID del proceso (para cualquier toast emitido por este proceso)
        let _ = SetCurrentProcessExplicitAppUserModelID(PCWSTR(aumid.as_ptr()));

        // 2. Acceso directo con AppUserModelID (solo app instalada)
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_s = exe.display().to_string();
        let lower = exe_s.to_lowercase();
        if lower.contains("\\target\\debug\\") || lower.contains("\\target\\release\\") {
            return Ok(());
        }
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        let lnk_path =
            format!(r"{appdata}\Microsoft\Windows\Start Menu\Programs\FocusFlow.lnk");

        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        link.SetPath(PCWSTR(w(&exe_s).as_ptr())).map_err(|e| e.to_string())?;
        link.SetIconLocation(PCWSTR(w(&exe_s).as_ptr()), 0)
            .map_err(|e| e.to_string())?;
        let store: IPropertyStore = link.cast().map_err(|e| e.to_string())?;
        let aumid_wide: Vec<u16> = AUMID.encode_utf16().chain(std::iter::once(0)).collect();
        // ManuallyDrop: el Drop del PROPVARIANT del crate llama PropVariantClear,
        // que intentaría liberar con CoTaskMemFree un puntero del heap de Rust
        // (el Vec) y abortaría el proceso. La variante nunca es dueña del buffer.
        let pv = core::mem::ManuallyDrop::new(propvariant_str(&aumid_wide));
        store.SetValue(&PKEY_AppUserModel_ID, &*pv).map_err(|e| e.to_string())?;
        store.Commit().map_err(|e| e.to_string())?;
        let persist: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        persist
            .Save(PCWSTR(w(&lnk_path).as_ptr()), true)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
