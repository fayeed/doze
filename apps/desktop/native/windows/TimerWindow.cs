using System.Text.Json.Nodes;
using System.Runtime.InteropServices;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using Windows.System;

namespace Doze.SettingsUi;

public sealed partial class TimerWindow : Window
{
    private readonly EngineBridge bridge;
    private bool awake;
    private bool usesDate;
    private bool pending;
    private bool closing;

    public TimerWindow(EngineBridge bridge)
    {
        this.bridge = bridge;
        InitializeComponent();
        WindowAppearance.Observe(this, root);
        SystemBackdrop = new MicaBackdrop { Kind = Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt };
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.ico"));
        var dpi = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        AppWindow.Resize(new SizeInt32((int)(540 * dpi), (int)(410 * dpi)));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.IsResizable = false;
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
        }
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(titlebar);
        cancel.Click += (_, _) => AppWindow.Hide();
        AutomationProperties.SetName(minutes, "Duration in minutes");
        AutomationProperties.SetName(date, "End date");
        AutomationProperties.SetName(time, "End time");
        start.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
        start.Click += async (_, _) => await StartAsync();
        root.KeyDown += async (_, args) =>
        {
            if (args.Key == VirtualKey.Escape) { AppWindow.Hide(); args.Handled = true; }
            else if (args.Key == VirtualKey.Enter && !pending) { args.Handled = true; await StartAsync(); }
        };
        AppWindow.Closing += (_, args) =>
        {
            if (closing || Environment.GetCommandLineArgs().Contains("--verify-ui")) return;
            args.Cancel = true; AppWindow.Hide();
        };
    }

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(nint window);

    public void SetTheme(string value) => WindowAppearance.Apply(root, value);
    public void StopAppearance() { closing = true; WindowAppearance.Stop(root); }

    public void Open(JsonObject message)
    {
        Configure(message["view"]!.GetValue<string>(), message["snapshot"]?["settings"]);
        AppWindow.Show(); Activate();
    }

    private void Configure(string view, JsonNode? settings)
    {
        awake = view.StartsWith("awake", StringComparison.Ordinal);
        usesDate = view.EndsWith("Time", StringComparison.Ordinal);
        pending = false; start.IsEnabled = true; error.IsOpen = false;
        Title = awake ? "Keep awake" : "Power timer";
        heading.Text = Title;
        minutes.Value = settings?[awake ? "defaultAwakeMinutes" : "defaultTimerMinutes"]?.GetValue<int>() ?? 30;
        var target = DateTimeOffset.Now.AddMinutes(minutes.Value);
        date.Date = target; date.MinDate = DateTimeOffset.Now; date.MaxDate = DateTimeOffset.Now.AddDays(7);
        time.Time = target.TimeOfDay;
        minutes.Visibility = usesDate ? Visibility.Collapsed : Visibility.Visible;
        pickers.Visibility = usesDate ? Visibility.Visible : Visibility.Collapsed;
        help.Text = usesDate ? "Choose a future local date and time, within seven days." : "From 1 minute to 7 days.";
    }

    internal static long DurationSeconds(double value)
    {
        if (!double.IsFinite(value) || value < 1 || value > 10080 || value != Math.Truncate(value))
            throw new ArgumentException("Enter whole minutes from 1 to 10080.");
        return checked((long)value * 60);
    }

    internal static long UntilSeconds(DateTime local, DateTimeOffset now)
    {
        if (TimeZoneInfo.Local.IsInvalidTime(local) || TimeZoneInfo.Local.IsAmbiguousTime(local))
            throw new ArgumentException("This local time is skipped or repeated by daylight saving. Choose another time.");
        var target = new DateTimeOffset(local, TimeZoneInfo.Local.GetUtcOffset(local));
        var seconds = (long)Math.Ceiling((target - now).TotalSeconds);
        if (seconds < 1 || seconds > 604800) throw new ArgumentException("Choose a future time within seven days.");
        return seconds;
    }

    private async Task StartAsync()
    {
        if (pending) return;
        try
        {
            var seconds = usesDate
                ? UntilSeconds((date.Date ?? throw new ArgumentException("Choose a date.")).Date + time.Time, DateTimeOffset.Now)
                : DurationSeconds(minutes.Value);
            pending = true; start.IsEnabled = false; error.IsOpen = false;
            await bridge.SendSessionAsync(awake, seconds);
        }
        catch (Exception failure) { ShowError(failure.Message); }
    }

    private void ShowError(string message)
    {
        pending = false; start.IsEnabled = true;
        error.Message = message; error.IsOpen = true;
    }

    public void Receive(JsonObject message)
    {
        if (!pending || message["command"]?.GetValue<string>() != (awake ? "awake" : "timer")) return;
        if (message["error"] is JsonValue failure) ShowError(failure.GetValue<string>());
        else if (message["snapshot"] is not null) { pending = false; start.IsEnabled = true; AppWindow.Hide(); }
    }

    public void Verify()
    {
        foreach (var view in new[] { "awakeDuration", "awakeTime", "timerDuration", "timerTime" }) Configure(view, null);
        if (SystemBackdrop is not MicaBackdrop) throw new InvalidOperationException("Timer does not use Mica.");
        if (DurationSeconds(30) != 1800 || DurationSeconds(10080) != 604800) throw new InvalidOperationException("Duration conversion failed.");
        foreach (var value in new[] { double.NaN, 0, -1, 1.5, 10081 })
        {
            try { DurationSeconds(value); } catch (ArgumentException) { continue; }
            throw new InvalidOperationException("Invalid duration accepted.");
        }
        var now = DateTimeOffset.Now;
        try { UntilSeconds(now.LocalDateTime.AddMinutes(-1), now); } catch (ArgumentException) { return; }
        throw new InvalidOperationException("Past end time accepted.");
    }

    public async Task RenderVerificationAsync(string directory)
    {
        try
        {
            AppWindow.Show(false);
            foreach (var theme in new[] { "light", "dark" })
                foreach (var view in new[] { "timerDuration", "timerTime" })
                {
                    SetTheme(theme); Configure(view, null); await Task.Delay(150);
                    // RenderTargetBitmap excludes the compositor's Mica backdrop.
                    root.Background = new SolidColorBrush(theme == "dark"
                        ? Windows.UI.Color.FromArgb(255, 32, 32, 32)
                        : Windows.UI.Color.FromArgb(255, 243, 243, 243));
                    await VisualVerification.SaveAsync(root, Path.Combine(directory, $"{view}-{theme}.png"));
                }
        }
        finally { root.Background = null; AppWindow.Hide(); }
    }
}
