//! Preserve native control behavior while painting text with compositor-compatible alpha.
use windows::{
    core::w,
    Win32::{
        Foundation::{BOOL, COLORREF, HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::*,
        UI::{
            Controls::*,
            Input::KeyboardAndMouse::{GetFocus, IsWindowEnabled},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
};

const SUBCLASS: usize = 0xD02E;
const COMPOSITED: windows::core::PCWSTR = w!("Doze.CompositedControl");
const LABEL: usize = 1;
const CHECKBOX: usize = 2;

pub(super) unsafe fn is_composited(hwnd: HWND) -> bool {
    !GetPropW(hwnd, COMPOSITED).is_invalid()
}

pub(super) unsafe fn install(hwnd: HWND) {
    let _ = EnumChildWindows(hwnd, Some(install_child), LPARAM(0));
}

unsafe extern "system" fn install_child(hwnd: HWND, _: LPARAM) -> BOOL {
    let mut class = [0u16; 32];
    let length = GetClassNameW(hwnd, &mut class);
    let class = String::from_utf16_lossy(&class[..length as usize]);
    let kind = if class.eq_ignore_ascii_case("static") {
        LABEL
    } else if class.eq_ignore_ascii_case("button")
        && GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & BS_TYPEMASK as u32 == BS_AUTOCHECKBOX as u32
    {
        CHECKBOX
    } else {
        return BOOL(1);
    };
    if SetWindowSubclass(hwnd, Some(control_proc), SUBCLASS, kind).as_bool() {
        let _ = SetPropW(hwnd, COMPOSITED, HANDLE(std::ptr::dangling_mut()));
    }
    let _ = InvalidateRect(hwnd, None, true);
    BOOL(1)
}

unsafe extern "system" fn control_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    kind: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        let _ = RemovePropW(hwnd, COMPOSITED);
        let _ = RemoveWindowSubclass(hwnd, Some(control_proc), SUBCLASS);
    }
    let glass = GetParent(hwnd).is_ok_and(|parent| super::appearance::enabled(parent));
    if glass && message == WM_ERASEBKGND {
        return LRESULT(1);
    }
    if glass && message == WM_PAINT && paint(hwnd, kind) {
        return LRESULT(0);
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

struct PaintResources {
    theme: HTHEME,
}
impl Drop for PaintResources {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseThemeData(self.theme);
            let _ = BufferedPaintUnInit();
        }
    }
}

unsafe fn paint(hwnd: HWND, kind: usize) -> bool {
    if BufferedPaintInit().is_err() {
        return false;
    }
    let theme = OpenThemeData(hwnd, w!("Button"));
    if theme.is_invalid() {
        let _ = BufferedPaintUnInit();
        return false;
    }
    let _resources = PaintResources { theme };
    let mut bounds = RECT::default();
    if GetClientRect(hwnd, &mut bounds).is_err() {
        return false;
    }
    let mut paint = PAINTSTRUCT::default();
    let target = BeginPaint(hwnd, &mut paint);
    let parameters = BP_PAINTPARAMS {
        cbSize: std::mem::size_of::<BP_PAINTPARAMS>() as u32,
        dwFlags: BPPF_ERASE,
        ..Default::default()
    };
    let mut dc = HDC::default();
    let buffer = BeginBufferedPaint(target, &bounds, BPBF_TOPDOWNDIB, Some(&parameters), &mut dc);
    if buffer == 0 {
        DefSubclassProc(
            hwnd,
            WM_PRINTCLIENT,
            WPARAM(target.0 as usize),
            LPARAM(PRF_CLIENT as isize),
        );
        let _ = EndPaint(hwnd, &paint);
        return true;
    }
    if !render(hwnd, kind, theme, dc, buffer, bounds) {
        // An unavailable theme renderer must leave readable native text, not invisible text.
        DefSubclassProc(
            hwnd,
            WM_PRINTCLIENT,
            WPARAM(dc.0 as usize),
            LPARAM(PRF_CLIENT as isize),
        );
        let _ = BufferedPaintSetAlpha(buffer, None, 255);
    }
    let _ = EndBufferedPaint(buffer, true);
    let _ = EndPaint(hwnd, &paint);
    true
}

unsafe fn render(
    hwnd: HWND,
    kind: usize,
    theme: HTHEME,
    dc: HDC,
    buffer: isize,
    bounds: RECT,
) -> bool {
    let font = SendMessageW(hwnd, WM_GETFONT, WPARAM(0), LPARAM(0));
    let old_font = SelectObject(dc, HGDIOBJ(font.0 as *mut _));
    let length = GetWindowTextLengthW(hwnd).max(0) as usize;
    let mut text = vec![0u16; length + 1];
    let length = GetWindowTextW(hwnd, &mut text).max(0) as usize;
    let mut text_bounds = bounds;
    let enabled = IsWindowEnabled(hwnd).as_bool();
    if kind == CHECKBOX {
        let mut metrics = TEXTMETRICW::default();
        let _ = GetTextMetricsW(dc, &mut metrics);
        let size = metrics.tmHeight.max(13);
        let top = (bounds.bottom - size) / 2;
        let check_bounds = RECT {
            left: 0,
            top,
            right: size,
            bottom: top + size,
        };
        let state = SendMessageW(hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0));
        let pressed =
            SendMessageW(hwnd, BM_GETSTATE, WPARAM(0), LPARAM(0)).0 as u32 & BST_PUSHED != 0;
        let base = if state.0 as u32 == BST_CHECKED.0 {
            5
        } else {
            1
        };
        let state = base
            + if !enabled {
                3
            } else if pressed {
                2
            } else {
                0
            };
        let _ = DrawThemeBackground(theme, dc, BP_CHECKBOX.0, state, &check_bounds, None);
        let _ = BufferedPaintSetAlpha(buffer, Some(&check_bounds), 255);
        text_bounds.left = size + 6;
    }
    let options = DTTOPTS {
        dwSize: std::mem::size_of::<DTTOPTS>() as u32,
        dwFlags: DTT_COMPOSITED | DTT_TEXTCOLOR,
        crText: COLORREF(GetSysColor(if enabled {
            COLOR_WINDOWTEXT
        } else {
            COLOR_GRAYTEXT
        })),
        ..Default::default()
    };
    let drawn = DrawThemeTextEx(
        theme,
        dc,
        BP_CHECKBOX.0,
        CBS_UNCHECKEDNORMAL.0,
        &text[..length],
        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        &mut text_bounds,
        Some(&options),
    )
    .is_ok();
    if kind == CHECKBOX && GetFocus() == hwnd {
        let _ = DrawFocusRect(dc, &text_bounds);
        // Only the focus outline is opaque; the space behind the label stays glass.
        for edge in [
            RECT {
                bottom: text_bounds.top + 1,
                ..text_bounds
            },
            RECT {
                top: text_bounds.bottom - 1,
                ..text_bounds
            },
            RECT {
                right: text_bounds.left + 1,
                ..text_bounds
            },
            RECT {
                left: text_bounds.right - 1,
                ..text_bounds
            },
        ] {
            let _ = BufferedPaintSetAlpha(buffer, Some(&edge), 255);
        }
    }
    SelectObject(dc, old_font);
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows::dialog_template::Template;
    use windows::Win32::Foundation::HINSTANCE;

    unsafe extern "system" fn test_proc(_: HWND, _: u32, _: WPARAM, _: LPARAM) -> isize {
        0
    }

    #[test]
    fn native_text_has_visible_alpha_without_an_opaque_label_background() {
        let mut form = Template::new("Glass rendering test", 240, 90);
        form.label_with_id(200, "Readable text over glass", 12, 12, 210);
        form.checkbox_at(101, "Native checkbox over glass", [12, 36, 210, 14]);
        let form = form.finish();
        unsafe {
            let hwnd = CreateDialogIndirectParamW(
                HINSTANCE::default(),
                form.as_ptr().cast(),
                HWND::default(),
                Some(test_proc),
                LPARAM(0),
            )
            .unwrap();
            install(hwnd);
            BufferedPaintInit().unwrap();
            let theme = OpenThemeData(hwnd, w!("Button"));
            assert!(!theme.is_invalid());
            let _resources = PaintResources { theme };
            for (id, kind) in [(200, LABEL), (101, CHECKBOX)] {
                let control = GetDlgItem(hwnd, id).unwrap();
                assert!(is_composited(control));
                let target = GetDC(control);
                let mut bounds = RECT::default();
                GetClientRect(control, &mut bounds).unwrap();
                let parameters = BP_PAINTPARAMS {
                    cbSize: std::mem::size_of::<BP_PAINTPARAMS>() as u32,
                    dwFlags: BPPF_ERASE,
                    ..Default::default()
                };
                let mut dc = HDC::default();
                let buffer = BeginBufferedPaint(
                    target,
                    &bounds,
                    BPBF_TOPDOWNDIB,
                    Some(&parameters),
                    &mut dc,
                );
                assert_ne!(buffer, 0);
                assert!(render(control, kind, theme, dc, buffer, bounds));
                let mut bits = std::ptr::null_mut();
                let mut stride = 0;
                GetBufferedPaintBits(buffer, &mut bits, &mut stride).unwrap();
                assert!(!bits.is_null() && stride > 0);
                let pixels =
                    std::slice::from_raw_parts(bits, stride as usize * bounds.bottom as usize);
                assert!(
                    pixels.iter().any(|pixel| pixel.rgbReserved > 0),
                    "Text must remain visible on glass"
                );
                assert!(
                    pixels.iter().filter(|pixel| pixel.rgbReserved == 0).count() > pixels.len() / 2,
                    "The label must not cover the material with a solid rectangle"
                );
                EndBufferedPaint(buffer, false).unwrap();
                ReleaseDC(control, target);
            }
            let checkbox = GetDlgItem(hwnd, 101).unwrap();
            SendMessageW(checkbox, BM_CLICK, WPARAM(0), LPARAM(0));
            assert_eq!(
                SendMessageW(checkbox, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 as u32,
                BST_CHECKED.0
            );
            DestroyWindow(hwnd).unwrap();
        }
    }
}
