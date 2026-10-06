import SwiftUI

/// The brand menu bar glyphs (icons/glyphs/doze-glyph-*.svg) on their 16-unit grid: the same
/// geometry the engine rasterizes for the status item's template images. They take the
/// current foreground style, like template images.
struct DozeGlyph: View {
    let state: String
    var size: CGFloat = 16

    private var unit: CGFloat { size / 16 }

    var body: some View {
        ZStack(alignment: .topLeading) {
            switch state {
            case "awake":
                Circle().frame(width: 13 * unit, height: 13 * unit).offset(x: 1.5 * unit, y: 1.5 * unit)
            case "attention":
                // The sun with a bite out of its top right, punched out of the layer...
                ZStack(alignment: .topLeading) {
                    Circle().frame(width: 12.5 * unit, height: 12.5 * unit).offset(x: 1.25 * unit, y: 2.25 * unit)
                    Circle().frame(width: 7.2 * unit, height: 7.2 * unit).offset(x: 9.4 * unit, y: -0.6 * unit)
                        .blendMode(.destinationOut)
                }
                .compositingGroup()
                // ...and the dot in the bite.
                Circle().frame(width: 4.2 * unit, height: 4.2 * unit).offset(x: 10.9 * unit, y: 0.9 * unit)
            case "countdown":
                ZStack(alignment: .topLeading) {
                    Circle().frame(width: 13 * unit, height: 13 * unit).offset(x: 1.5 * unit, y: 1.5 * unit)
                }
                .frame(width: size, height: size, alignment: .topLeading)
                .mask(HorizonBands(unit: unit))
            default:
                Circle().strokeBorder(lineWidth: 1.5 * unit)
                    .frame(width: 13 * unit, height: 13 * unit).offset(x: 1.5 * unit, y: 1.5 * unit)
            }
        }
        .frame(width: size, height: size, alignment: .topLeading)
        .accessibilityHidden(true)
    }
}

/// The setting sun's top and its two horizon bands, as a mask.
private struct HorizonBands: View {
    let unit: CGFloat
    var body: some View {
        ZStack(alignment: .topLeading) {
            Rectangle().frame(width: 16 * unit, height: 9.5 * unit)
            Rectangle().frame(width: 16 * unit, height: 1.5 * unit).offset(y: 10.5 * unit)
            Rectangle().frame(width: 16 * unit, height: 3 * unit).offset(y: 13 * unit)
        }
        .frame(width: 16 * unit, height: 16 * unit, alignment: .topLeading)
    }
}
