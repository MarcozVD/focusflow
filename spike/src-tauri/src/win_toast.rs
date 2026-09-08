//! Identidad de las notificaciones de Windows.
//!
//! `tauri-plugin-notification` envía los toasts con el AppUserModelID
//! `com.focusflow.spike` (el `identifier` del bundle). Windows muestra el
//! nombre e icono de la app SOLO si ese AUMID está registrado; si no, el
//! sistema degrada el toast y lo muestra como si viniera de PowerShell.
//!
//! El registro se hace por la vía canónica: la clave de registro
//! `HKCU\Software\Classes\AppUserModelId\<AUMID>` con `DisplayName` e
//! `IconUri`. Es la que Windows consulta para atribuir el toast, y funciona
//! aunque el acceso directo del menú Inicio no esté indexado todavía.
//! Además se crea/actualiza `FocusFlow.lnk` en el menú Inicio con la
//! propiedad `System.AppUserModel.ID` = `com.focusflow.spike` (refuerzo).

#![cfg(windows)]

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

use windows::core::{Interface, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{IShellLinkW, SetCurrentProcessExplicitAppUserModelID, ShellLink};

/// Debe coincidir con `identifier` en tauri.conf.json.
pub const AUMID: &str = "com.focusflow.spike";

/// Clave de registro donde Windows busca el AUMID para atribuir los toasts.
const AUMID_REG_KEY: &str = r"Software\Classes\AppUserModelId\com.focusflow.spike";

fn w(s: &str) -> HSTRING {
    HSTRING::from(s)
}

/// PROPVARIANT VT_LPWSTR construido a mano.
/// `SetValue` copia la cadena, así que el buffer `wide` solo tiene que vivir
/// hasta que vuelve la llamada, y el `ManuallyDrop` externo evita que el
/// Drop del PROPVARIANT libere el buffer de Rust con CoTaskMemFree.
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

/// Registra el AUMID en `HKCU\Software\Classes\AppUserModelId\<AUMID>` con el
/// nombre visible y el icono del exe. Sin esto, los toasts en segundo plano
/// se atribuyen a PowerShell. Idempotente: se puede llamar en cada arranque.
fn register_registry_aumid(exe_path: &str) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    // create_subkey abre/crea con KEY_READ|KEY_WRITE implícito
    let (key, _disposition) = hkcu
        .create_subkey(AUMID_REG_KEY)
        .map_err(|e| format!("create_subkey: {e}"))?;
    key.set_value("DisplayName", &"FocusFlow")
        .map_err(|e| format!("DisplayName: {e}"))?;
    // IconUri espera una ruta de archivo; el formato file:/// también funciona,
    // pero la ruta plana al exe es la más compatible con el Shell.
    key.set_value("IconUri", &exe_path)
        .map_err(|e| format!("IconUri: {e}"))?;
    if let Some(dir) = std::path::Path::new(exe_path).parent() {
        if let Some(dir_s) = dir.to_str() {
            let _ = key.set_value("InstallLocation", &dir_s);
        }
    }
    Ok(())
}

/// Idempotente y barato: se puede llamar en cada arranque. En builds de
/// desarrollo (target\debug / target\release) solo fija el AUMID del proceso
/// y registra la clave de registro; no toca el menú Inicio.
pub fn ensure_toast_identity() -> Result<(), String> {
    unsafe {
        let aumid = w(AUMID);
        // 1. AUMID del proceso (para cualquier toast emitido por este proceso)
        let _ = SetCurrentProcessExplicitAppUserModelID(PCWSTR(aumid.as_ptr()));

        // 2. Registro canónico del AUMID (DisplayName + IconUri)
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_s = exe.display().to_string();
        register_registry_aumid(&exe_s)?;
        append_identity_log(&format!("toast_aumid_registered key={AUMID_REG_KEY}"));

        // 3. Acceso directo con AppUserModelID (solo app instalada)
        let lower = exe_s.to_lowercase();
        if lower.contains("\\target\\debug\\") || lower.contains("\\target\\release\\") {
            return Ok(());
        }
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        let lnk_path = format!(r"{appdata}\Microsoft\Windows\Start Menu\Programs\FocusFlow.lnk");

        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        link.SetPath(PCWSTR(w(&exe_s).as_ptr()))
            .map_err(|e| e.to_string())?;
        link.SetIconLocation(PCWSTR(w(&exe_s).as_ptr()), 0)
            .map_err(|e| e.to_string())?;
        let store: IPropertyStore = link.cast().map_err(|e| e.to_string())?;
        let aumid_wide: Vec<u16> = AUMID.encode_utf16().chain(std::iter::once(0)).collect();
        // ManuallyDrop: el Drop del PROPVARIANT del crate llama PropVariantClear,
        // que intentaría liberar con CoTaskMemFree un puntero del heap de Rust
        // (el Vec) y abortaría el proceso. La variante nunca es dueña del buffer.
        let pv = core::mem::ManuallyDrop::new(propvariant_str(&aumid_wide));
        store
            .SetValue(&PKEY_AppUserModel_ID, &*pv)
            .map_err(|e| e.to_string())?;
        store.Commit().map_err(|e| e.to_string())?;
        let persist: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        persist
            .Save(PCWSTR(w(&lnk_path).as_ptr()), true)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Log en la misma ruta que `append_log` del crate (temp\focusflow-spike\spike.log).
fn append_identity_log(line: &str) {
    let dir = std::env::temp_dir().join("focusflow-spike");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("spike.log"))
    {
        use std::io::Write;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(f, "[{ts}] {line}");
    }
}

/// Verifica (para tests/diagnóstico) si el AUMID está registrado en HKCU.
pub fn aumid_registered() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(AUMID_REG_KEY, KEY_READ | KEY_WRITE)
        .is_ok()
}
