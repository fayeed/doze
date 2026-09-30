//! Small, antialiased line icons carried by native menu items.
use tauri::{
    image::Image,
    menu::{IconMenuItem, Submenu},
};

#[derive(Clone, Copy)]
pub(crate) enum Glyph {
    Awake,
    Timer,
    Stop,
    Add,
    Settings,
    Quick,
    Info,
    Help,
    Preview,
    Quit,
}

pub(crate) fn item(
    app: &tauri::App,
    id: &str,
    text: &str,
    glyph: Glyph,
) -> tauri::Result<IconMenuItem<tauri::Wry>> {
    let accelerator = match id {
        "settings" => Some("CmdOrCtrl+,"),
        "quit" => Some("CmdOrCtrl+Q"),
        _ => None,
    };
    let item = IconMenuItem::with_id(app, id, text, true, Some(image(glyph)), accelerator)?;
    #[cfg(target_os = "macos")]
    if let Some(icon) = native(glyph) {
        item.set_native_icon(Some(icon))?;
    }
    Ok(item)
}

pub(crate) fn submenu(
    app: &tauri::App,
    id: &str,
    text: &str,
    glyph: Glyph,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let menu = Submenu::with_id_and_icon(app, id, text, true, Some(image(glyph)))?;
    #[cfg(target_os = "macos")]
    if let Some(icon) = native(glyph) {
        menu.set_native_icon(Some(icon))?;
    }
    Ok(menu)
}

#[cfg(target_os = "macos")]
fn native(glyph: Glyph) -> Option<tauri::menu::NativeIcon> {
    use tauri::menu::NativeIcon;
    Some(match glyph {
        Glyph::Settings => NativeIcon::PreferencesGeneral,
        Glyph::Quick => NativeIcon::Advanced,
        Glyph::Info | Glyph::Help => NativeIcon::Info,
        Glyph::Add => NativeIcon::Add,
        Glyph::Stop => NativeIcon::StopProgress,
        Glyph::Preview => NativeIcon::QuickLook,
        _ => return None,
    })
}

enum Stroke {
    Line([f64; 4]),
    Circle(f64, f64, f64),
    Arc(f64, f64, f64, f64, f64),
}
impl Stroke {
    fn distance(&self, x: f64, y: f64) -> f64 {
        match *self {
            Self::Line([ax, ay, bx, by]) => {
                let (dx, dy) = (bx - ax, by - ay);
                let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                (x - ax - t * dx).hypot(y - ay - t * dy)
            }
            Self::Circle(cx, cy, r) => ((x - cx).hypot(y - cy) - r).abs(),
            Self::Arc(cx, cy, r, start, end) => {
                let angle = (y - cy).atan2(x - cx).rem_euclid(std::f64::consts::TAU);
                if (start..=end).contains(&angle) {
                    ((x - cx).hypot(y - cy) - r).abs()
                } else {
                    [start, end]
                        .into_iter()
                        .map(|a| (x - cx - r * a.cos()).hypot(y - cy - r * a.sin()))
                        .fold(f64::INFINITY, f64::min)
                }
            }
        }
    }
}

fn image(glyph: Glyph) -> Image<'static> {
    use Stroke::{Arc, Circle, Line};
    let strokes = match glyph {
        Glyph::Awake => vec![
            Circle(12.0, 12.0, 4.0),
            Line([12.0, 2.0, 12.0, 4.0]),
            Line([12.0, 20.0, 12.0, 22.0]),
            Line([2.0, 12.0, 4.0, 12.0]),
            Line([20.0, 12.0, 22.0, 12.0]),
            Line([5.0, 5.0, 6.5, 6.5]),
            Line([17.5, 17.5, 19.0, 19.0]),
            Line([5.0, 19.0, 6.5, 17.5]),
            Line([17.5, 6.5, 19.0, 5.0]),
        ],
        Glyph::Timer => vec![
            Circle(12.0, 13.0, 8.0),
            Line([12.0, 13.0, 12.0, 8.0]),
            Line([12.0, 13.0, 16.0, 15.0]),
            Line([9.0, 2.0, 15.0, 2.0]),
            Line([12.0, 2.0, 12.0, 5.0]),
        ],
        Glyph::Stop => vec![
            Line([6.0, 6.0, 18.0, 6.0]),
            Line([18.0, 6.0, 18.0, 18.0]),
            Line([18.0, 18.0, 6.0, 18.0]),
            Line([6.0, 18.0, 6.0, 6.0]),
        ],
        Glyph::Add => vec![
            Circle(12.0, 12.0, 9.0),
            Line([8.0, 12.0, 16.0, 12.0]),
            Line([12.0, 8.0, 12.0, 16.0]),
        ],
        Glyph::Settings | Glyph::Quick => vec![
            Line([4.0, 6.0, 20.0, 6.0]),
            Line([4.0, 12.0, 20.0, 12.0]),
            Line([4.0, 18.0, 20.0, 18.0]),
            Circle(8.0, 6.0, 2.0),
            Circle(16.0, 12.0, 2.0),
            Circle(10.0, 18.0, 2.0),
        ],
        Glyph::Info | Glyph::Help => vec![
            Circle(12.0, 12.0, 9.0),
            Circle(12.0, 7.0, 0.3),
            Line([12.0, 11.0, 12.0, 17.0]),
        ],
        Glyph::Preview => vec![
            Line([8.0, 5.0, 19.0, 12.0]),
            Line([19.0, 12.0, 8.0, 19.0]),
            Line([8.0, 19.0, 8.0, 5.0]),
        ],
        Glyph::Quit => vec![
            Arc(12.0, 12.0, 8.0, 0.0, 4.0),
            Arc(12.0, 12.0, 8.0, 5.4, std::f64::consts::TAU),
            Line([12.0, 2.0, 12.0, 11.0]),
        ],
    };
    let mut pixels = vec![0; 24 * 24 * 4];
    for y in 0..24 {
        for x in 0..24 {
            let distance = strokes
                .iter()
                .map(|stroke| stroke.distance(x as f64 + 0.5, y as f64 + 0.5))
                .fold(f64::INFINITY, f64::min);
            let alpha = ((1.3 - distance).clamp(0.0, 1.0) * 255.0) as u8;
            let offset = (y * 24 + x) * 4;
            // Neutral slate stays visible against both light and dark native menus.
            pixels[offset..offset + 4].copy_from_slice(&[125, 132, 145, alpha]);
        }
    }
    Image::new_owned(pixels, 24, 24)
}
