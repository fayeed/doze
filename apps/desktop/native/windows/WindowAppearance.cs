using Microsoft.UI;
using Microsoft.UI.Xaml;
using System.Runtime.CompilerServices;
using Windows.UI.ViewManagement;

namespace Doze.SettingsUi;

internal sealed class WindowAppearance
{
    private static readonly ConditionalWeakTable<FrameworkElement, WindowAppearance> appearances = new();
    private readonly UISettings system = new();
    private string theme = "system";
    private Windows.Foundation.TypedEventHandler<UISettings, object>? colorChanged;
    private Windows.Foundation.TypedEventHandler<FrameworkElement, object>? actualChanged;

    public static void Stop(FrameworkElement root)
    {
        if (!appearances.TryGetValue(root, out var appearance)) return;
        if (appearance.colorChanged is not null) appearance.system.ColorValuesChanged -= appearance.colorChanged;
        if (appearance.actualChanged is not null) root.ActualThemeChanged -= appearance.actualChanged;
        appearance.colorChanged = null;
        appearance.actualChanged = null;
    }

    public static void Apply(FrameworkElement root, string theme)
    {
        var appearance = appearances.GetValue(root, _ => new WindowAppearance());
        appearance.theme = theme;
        var background = appearance.system.GetColorValue(UIColorType.Background);
        root.RequestedTheme = theme switch
        {
            "light" => ElementTheme.Light,
            "dark" => ElementTheme.Dark,
            _ => background.R + background.G + background.B < 384 ? ElementTheme.Dark : ElementTheme.Light
        };
    }

    public static void Observe(Window window, FrameworkElement root)
    {
        var appearance = appearances.GetValue(root, _ => new WindowAppearance());
        appearance.colorChanged = (_, _) => root.DispatcherQueue.TryEnqueue(() =>
        {
            if (appearance.colorChanged is not null) Apply(root, appearance.theme);
        });
        appearance.system.ColorValuesChanged += appearance.colorChanged;
        void UpdateCaption()
        {
            window.AppWindow.TitleBar.ButtonForegroundColor = root.ActualTheme == ElementTheme.Dark ? Colors.White : Colors.Black;
            window.AppWindow.TitleBar.ButtonBackgroundColor = Colors.Transparent;
            window.AppWindow.TitleBar.ButtonInactiveBackgroundColor = Colors.Transparent;
        }
        appearance.actualChanged = (_, _) => UpdateCaption();
        root.ActualThemeChanged += appearance.actualChanged;
        window.Closed += (_, _) => Stop(root);
        Apply(root, appearance.theme);
        UpdateCaption();
    }
}
