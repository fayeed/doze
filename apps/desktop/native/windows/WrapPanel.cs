using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Foundation;

namespace Doze.SettingsUi;

/// Lays children out left to right and wraps them onto new lines, so rows of buttons never
/// clip in a narrow window or at large text sizes.
internal sealed partial class WrapPanel : Panel
{
    public double Spacing { get; set; } = 8;

    protected override Size MeasureOverride(Size available)
    {
        double x = 0, y = 0, line = 0, width = 0;
        foreach (var child in Children.Where(child => child.Visibility == Visibility.Visible))
        {
            child.Measure(new Size(available.Width, double.PositiveInfinity));
            var size = child.DesiredSize;
            if (x > 0 && x + size.Width > available.Width) { y += line + Spacing; x = 0; line = 0; }
            x += size.Width + Spacing;
            line = Math.Max(line, size.Height);
            width = Math.Max(width, x - Spacing);
        }
        return new Size(double.IsInfinity(available.Width) ? width : Math.Min(width, available.Width), y + line);
    }

    protected override Size ArrangeOverride(Size final)
    {
        double x = 0, y = 0, line = 0;
        foreach (var child in Children.Where(child => child.Visibility == Visibility.Visible))
        {
            var size = child.DesiredSize;
            if (x > 0 && x + size.Width > final.Width) { y += line + Spacing; x = 0; line = 0; }
            child.Arrange(new Rect(x, y, size.Width, size.Height));
            x += size.Width + Spacing;
            line = Math.Max(line, size.Height);
        }
        return final;
    }
}
