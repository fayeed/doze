//! Full-window DWM materials with alpha-correct native label and checkbox painting.
use windows::{
    core::w,
    Win32::{
        Foundation::{HANDLE, HWND, LPARAM, RECT, WPARAM},
        Graphics::{
            Dwm::*,
            Gdi::{FillRect, GetStockObject, InvalidateRect, BLACK_BRUSH, HBRUSH, HDC},
        },
        UI::{
            Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
            Controls::MARGINS,
            WindowsAndMessaging::*,
        },
    },
};

const MATERIAL: windows::core::PCWSTR = w!("Doze.NativeMaterial");

#[derive(Clone, Copy)]
pub(super) enum Surface {
    Persistent,
}

/// Material support is optional: Windows 10 and accessibility modes keep ordinary dialogs.
pub(super) unsafe fn apply(hwnd: HWND, surface: Surface) {
    let _ = RemovePropW(hwnd, MATERIAL);
    let mut contrast = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    let accessible = SystemParametersInfoW(
        SPI_GETHIGHCONTRAST,
        contrast.cbSize,
        Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
    )
    .is_err()
        || contrast.dwFlags.contains(HCF_HIGHCONTRASTON);
    let backdrop = if accessible {
        DWMSBT_NONE
    } else {
        match surface {
            Surface::Persistent => DWMSBT_MAINWINDOW,
        }
    };
    let supported = attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &backdrop);
    let _ = attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND);
    let enabled = supported && !accessible;
    let margins = if enabled {
        MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        }
    } else {
        MARGINS::default()
    };
    if DwmExtendFrameIntoClientArea(hwnd, &margins).is_ok() && enabled {
        // A small integer marker, never a pointer to owned memory.
        let _ = SetPropW(hwnd, MATERIAL, HANDLE(std::ptr::dangling_mut()));
    }
    super::glass_controls::install(hwnd);
    let _ = InvalidateRect(hwnd, None, true);
}

unsafe fn attribute<T>(hwnd: HWND, key: DWMWINDOWATTRIBUTE, value: &T) -> bool {
    DwmSetWindowAttribute(
        hwnd,
        key,
        (value as *const T).cast(),
        std::mem::size_of::<T>() as u32,
    )
    .is_ok()
}

/// Handle appearance messages before borrowing a dialog's engine/request context.
pub(super) unsafe fn message(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    surface: Surface,
) -> Option<isize> {
    match message {
        WM_THEMECHANGED | WM_SETTINGCHANGE | WM_DWMCOMPOSITIONCHANGED => {
            apply(hwnd, surface);
            None
        }
        WM_NCDESTROY => {
            let _ = RemovePropW(hwnd, MATERIAL);
            None
        }
        WM_CTLCOLORSTATIC if !GetPropW(hwnd, MATERIAL).is_invalid() => {
            let control = HWND(lparam.0 as *mut _);
            if super::glass_controls::is_composited(control) {
                return Some(GetStockObject(BLACK_BRUSH).0 as isize);
            }
            None
        }
        WM_ERASEBKGND if !GetPropW(hwnd, MATERIAL).is_invalid() => {
            let dc = HDC(wparam.0 as *mut _);
            let mut bounds = RECT::default();
            if GetClientRect(hwnd, &mut bounds).is_err() {
                return None;
            }
            // Black pixels in the extended frame reveal the compositor's real material.
            FillRect(dc, &bounds, HBRUSH(GetStockObject(BLACK_BRUSH).0));
            Some(1)
        }
        _ => None,
    }
}

pub(super) unsafe fn enabled(hwnd: HWND) -> bool {
    !GetPropW(hwnd, MATERIAL).is_invalid()
}
