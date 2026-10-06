using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Markup;

namespace Doze.SettingsUi;

/// The brand tray glyphs (icons/glyphs/doze-glyph-*.svg) as vector shapes, so they follow the
/// theme like the tray's template images: hollow sun, whole sun, sun with a dot, banded sun.
internal static class Glyphs
{
    private const string Ns = "xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\"";

    private static string Shapes(string state, string brush) => state switch
    {
        "awake" => $"<Ellipse Canvas.Left=\"1.5\" Canvas.Top=\"1.5\" Width=\"13\" Height=\"13\" Fill=\"{brush}\" />",
        "attention" => $"<Path Fill=\"{brush}\" Data=\"M13.45,6.57 A6.25,6.25 0 1 1 9.43,2.55 A3.6,3.6 0 0 0 13.45,6.57 Z\" />"
            + $"<Ellipse Canvas.Left=\"10.9\" Canvas.Top=\"0.9\" Width=\"4.2\" Height=\"4.2\" Fill=\"{brush}\" />",
        "countdown" => $"<Path Fill=\"{brush}\" Data=\"M1.68,9.5 A6.5,6.5 0 1 1 14.32,9.5 Z M2,10.5 L14,10.5 A6.5,6.5 0 0 1 13.12,12 L2.88,12 A6.5,6.5 0 0 1 2,10.5 Z M12.15,13 A6.5,6.5 0 0 1 3.85,13 Z\" />",
        _ => $"<Ellipse Canvas.Left=\"2.25\" Canvas.Top=\"2.25\" Width=\"11.5\" Height=\"11.5\" Stroke=\"{brush}\" StrokeThickness=\"1.5\" />",
    };

    /// A glyph at `size` effective pixels, drawn in the given theme brush.
    public static FrameworkElement Create(string state, double size, string brush = "{ThemeResource TextFillColorPrimaryBrush}")
    {
        var element = (FrameworkElement)XamlReader.Load(
            $"<Viewbox {Ns} Width=\"{size}\" Height=\"{size}\"><Canvas Width=\"16\" Height=\"16\">{Shapes(state, brush)}</Canvas></Viewbox>");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(element, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return element;
    }
}

/// `doze` on the command line: Windows finds doze-cli.exe once its folder is on the user's PATH.
internal static class CommandLineInstall
{
    private static IEnumerable<string> UserPath() =>
        (Environment.GetEnvironmentVariable("Path", EnvironmentVariableTarget.User) ?? "")
            .Split(';', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);

    public static bool OnPath(string folder) =>
        UserPath().Any(entry => string.Equals(entry.TrimEnd('\\'), folder.TrimEnd('\\'), StringComparison.OrdinalIgnoreCase));

    /// Adds the folder for the current user only. Windows broadcasts the change, so new
    /// terminals pick it up; administrator rights are not needed.
    public static void AddToPath(string folder)
    {
        if (OnPath(folder)) return;
        var entries = UserPath().Append(folder);
        Environment.SetEnvironmentVariable("Path", string.Join(';', entries), EnvironmentVariableTarget.User);
    }
}
