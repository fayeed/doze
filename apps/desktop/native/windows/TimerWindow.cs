using System.Text.Json.Nodes;
using System.Runtime.InteropServices;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics;
using Windows.System;

namespace Doze.SettingsUi;

// Custom Keep Awake or Power Timer: an action, a duration or an end time, and a live preview
// of when it ends. The engine validates the request; this window only describes it.
public sealed partial class TimerWindow : Window
{
    private readonly EngineBridge bridge;
    private readonly DispatcherTimer clock = new() { Interval = TimeSpan.FromSeconds(1) };
    private bool awake;
    private bool remember;
    private bool usesDate;
    private bool pending;
    private bool closing;
    private const int FormWidth = 480;
    private static readonly int[] PresetMinutes = [15, 30, 60, 120, 240, 480];

    public TimerWindow(EngineBridge bridge)
    {
        this.bridge = bridge;
        InitializeComponent();
        WindowAppearance.Observe(this, root);
        SystemBackdrop = new MicaBackdrop { Kind = Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt };
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.ico"));
        var icon = Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.png");
        if (File.Exists(icon)) titleIcon.Source = new BitmapImage(new Uri(icon)) { DecodePixelWidth = 32 };
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.IsResizable = false;
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
        }
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(titlebar);
        foreach (var preset in PresetMinutes)
        {
            var button = new Button { Content = Labels.Preset(preset), MinWidth = 52 };
            AutomationProperties.SetName(button, "Set duration to " + Labels.Minutes(preset));
            button.Click += (_, _) => minutes.Value = preset;
            presets.Children.Add(button);
        }
        cancel.Click += (_, _) => Hide();
        start.Click += async (_, _) => await StartAsync();
        minutes.ValueChanged += (_, _) => UpdatePreview();
        date.DateChanged += (_, _) => UpdatePreview();
        time.TimeChanged += (_, _) => UpdatePreview();
        action.SelectionChanged += (_, _) => UpdatePreview();
        clock.Tick += (_, _) => UpdatePreview();
        error.RegisterPropertyChangedCallback(InfoBar.IsOpenProperty, (_, _) => FitToContent());
        root.KeyDown += async (_, args) =>
        {
            if (args.Key == VirtualKey.Escape) { Hide(); args.Handled = true; }
            else if (args.Key == VirtualKey.Enter && !pending && args.OriginalSource is not Button) { args.Handled = true; await StartAsync(); }
        };
        AppWindow.Closing += (_, args) =>
        {
            if (closing || Environment.GetCommandLineArgs().Contains("--verify-ui")) return;
            args.Cancel = true; Hide();
        };
    }

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(nint window);

    private double Scale => GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;

    public void SetTheme(string value) => WindowAppearance.Apply(root, value);
    public void StopAppearance() { closing = true; clock.Stop(); WindowAppearance.Stop(root); }

    public void Open(JsonObject message)
    {
        Configure(message["view"]!.GetValue<string>(), message["snapshot"] as JsonObject);
        AppWindow.Show(); Activate();
        clock.Start();
        (usesDate ? (Control)date : minutes).Focus(FocusState.Programmatic);
    }

    private void Hide() { clock.Stop(); AppWindow.Hide(); }

    private void Configure(string view, JsonObject? snapshot)
    {
        awake = view.StartsWith("awake", StringComparison.Ordinal);
        usesDate = view.EndsWith("Time", StringComparison.Ordinal);
        pending = false; start.IsEnabled = true; error.IsOpen = false;
        var settings = snapshot?["settings"];
        Title = awake ? "Doze · Keep Awake" : "Doze · Power Timer";
        titleText.Text = Title;
        heading.Text = awake ? "Keep Awake" : "Power Timer";
        help.Text = awake
            ? usesDate ? "The computer stays awake until the time you choose, then normal sleep settings apply again."
                       : "The computer stays awake, then normal sleep settings apply again."
            : "A final warning lets you cancel or snooze before the action runs.";
        start.Content = awake ? "Keep Awake" : "Start Timer";

        action.Items.Clear();
        var actions = (snapshot?["actions"] as JsonArray)?.Select(a => a!.GetValue<string>()).ToList() ?? ["sleep"];
        foreach (var name in actions) action.Items.Add(new ComboBoxItem { Content = Labels.Action(name), Tag = name });
        var selected = snapshot?["session"]?["selectedAction"]?.GetValue<string>() ?? settings?["defaultAction"]?.GetValue<string>() ?? "sleep";
        action.SelectedItem = action.Items.OfType<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag == selected) ?? action.Items.FirstOrDefault();
        action.Visibility = awake ? Visibility.Collapsed : Visibility.Visible;

        // Remember my last custom duration: a custom Keep Awake starts from the last one used.
        var remembered = awake && settings?["rememberLastCustomDuration"]?.GetValue<bool>() == true
            ? settings?["lastCustomAwakeMinutes"]?.GetValue<int?>() : null;
        minutes.Value = remembered ?? settings?[awake ? "defaultAwakeMinutes" : "defaultTimerMinutes"]?.GetValue<int>() ?? 30;
        remember = awake && settings?["rememberLastCustomDuration"]?.GetValue<bool>() == true;
        var target = DateTimeOffset.Now.AddMinutes(minutes.Value);
        date.MinDate = DateTimeOffset.Now.Date; date.MaxDate = DateTimeOffset.Now.AddDays(7);
        date.Date = target; time.Time = new TimeSpan(target.Hour, target.Minute, 0);
        durationPanel.Visibility = usesDate ? Visibility.Collapsed : Visibility.Visible;
        pickers.Visibility = usesDate ? Visibility.Visible : Visibility.Collapsed;
        UpdatePreview();
        FitToContent();
    }

    private string? SelectedAction => (action.SelectedItem as ComboBoxItem)?.Tag as string;

    /// "Ends 11:42 PM" for a duration, "In 1h 5m" for an end time, or what to fix.
    internal string PreviewText(DateTimeOffset now)
    {
        try
        {
            var seconds = Seconds(now);
            if (usesDate) return "In " + Labels.Remaining(seconds);
            var end = now.AddSeconds(seconds).ToLocalTime();
            var when = end.Date == now.Date ? end.ToString("t")
                : end.Date == now.Date.AddDays(1) ? "tomorrow at " + end.ToString("t")
                : end.ToString("dddd") + " at " + end.ToString("t");
            return (awake ? "Ends " : $"{Labels.Action(SelectedAction)} at ") + when + (awake ? "" : ", after the final warning");
        }
        catch (ArgumentException failure) { return failure.Message; }
    }

    private void UpdatePreview() => preview.Text = PreviewText(DateTimeOffset.Now);

    private long Seconds(DateTimeOffset now) => usesDate
        ? UntilSeconds((date.Date ?? throw new ArgumentException("Choose a date.")).Date + time.Time, now)
        : DurationSeconds(minutes.Value);

    internal static long DurationSeconds(double value)
    {
        if (!double.IsFinite(value) || value < 1 || value > 10080 || value != Math.Truncate(value))
            throw new ArgumentException("Choose whole minutes from 1 minute to 7 days.");
        return checked((long)value * 60);
    }

    internal static long UntilSeconds(DateTime local, DateTimeOffset now)
    {
        if (TimeZoneInfo.Local.IsInvalidTime(local) || TimeZoneInfo.Local.IsAmbiguousTime(local))
            throw new ArgumentException("This local time is skipped or repeated by daylight saving. Choose another time.");
        var target = new DateTimeOffset(local, TimeZoneInfo.Local.GetUtcOffset(local));
        var seconds = (long)Math.Ceiling((target - now).TotalSeconds);
        if (seconds < 60) throw new ArgumentException("Choose a time at least a minute from now.");
        if (seconds > 604800) throw new ArgumentException("Choose a time within seven days.");
        return seconds;
    }

    /// The command this form sends: awake {seconds} or timer {seconds, action}.
    internal (string Command, JsonObject Fields) Request(DateTimeOffset now)
    {
        var fields = new JsonObject { ["seconds"] = Seconds(now) };
        if (!awake) fields["action"] = SelectedAction ?? throw new ArgumentException("Choose an action.");
        return (awake ? "awake" : "timer", fields);
    }

    private async Task StartAsync()
    {
        if (pending) return;
        try
        {
            var (command, fields) = Request(DateTimeOffset.Now);
            pending = true; start.IsEnabled = false; error.IsOpen = false;
            await bridge.SendCommandAsync(command, fields);
            if (remember && durationPanel.Visibility == Visibility.Visible)
                await bridge.SendCommandAsync("set", new JsonObject { ["key"] = "lastCustomAwakeMinutes", ["value"] = (int)minutes.Value });
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
        else if (message["snapshot"] is not null) { pending = false; start.IsEnabled = true; Hide(); }
    }

    /// Sizes the window to its form, within the work area, so no mode leaves empty space.
    private void FitToContent()
    {
        form.Measure(new Windows.Foundation.Size(FormWidth, double.PositiveInfinity));
        footer.Measure(new Windows.Foundation.Size(FormWidth, double.PositiveInfinity));
        var height = 40 + form.DesiredSize.Height + footer.DesiredSize.Height;
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var size = new SizeInt32(Math.Min((int)(FormWidth * Scale), area.Width), Math.Min((int)Math.Ceiling(height * Scale), area.Height * 92 / 100));
        if (AppWindow.IsVisible) AppWindow.Resize(size);
        else AppWindow.MoveAndResize(new RectInt32(area.X + (area.Width - size.Width) / 2, area.Y + (area.Height - size.Height) / 2, size.Width, size.Height));
    }

    public void Verify(JsonObject? snapshot)
    {
        if (SystemBackdrop is not MicaBackdrop) throw new InvalidOperationException("Timer does not use Mica.");
        var sample = snapshot?.DeepClone().AsObject() ?? new JsonObject();
        sample["actions"] = new JsonArray("sleep", "shutdown", "displayOff");
        sample["session"] = new JsonObject { ["selectedAction"] = "displayOff" };
        var now = DateTimeOffset.Now;
        foreach (var view in new[] { "awakeDuration", "awakeTime", "timerDuration", "timerTime" })
        {
            Configure(view, sample);
            if ((action.Visibility == Visibility.Visible) == awake) throw new InvalidOperationException($"{view}: the action picker is shown for the wrong kind.");
            if ((string)start.Content != (awake ? "Keep Awake" : "Start Timer")) throw new InvalidOperationException($"{view}: wrong start button.");
            var text = PreviewText(now);
            if (!(usesDate ? text.StartsWith("In ") : text.Contains(" at ") || text.StartsWith("Ends ")))
                throw new InvalidOperationException($"{view}: preview reads \"{text}\".");
        }
        Configure("timerDuration", sample);
        if (SelectedAction != "displayOff") throw new InvalidOperationException("The timer did not default to the selected action.");
        // Press the 4h preset as a screen reader would.
        new Microsoft.UI.Xaml.Automation.Peers.ButtonAutomationPeer((Button)presets.Children[4]).Invoke();
        var (command, fields) = Request(now);
        if (command != "timer" || fields["seconds"]!.GetValue<long>() != 14400 || fields["action"]!.GetValue<string>() != "displayOff")
            throw new InvalidOperationException("The timer request lost its action or duration.");
        if (!PreviewText(now).StartsWith("Turn display off at ")) throw new InvalidOperationException("The preview does not name the action.");
        Configure("awakeDuration", sample);
        if (Request(now).Fields.ContainsKey("action")) throw new InvalidOperationException("Keep Awake sent an action.");
        if (presets.Children.Count != 6 || AutomationProperties.GetName(presets.Children[0]) != "Set duration to 15 minutes")
            throw new InvalidOperationException("Duration presets are missing.");
        if (DurationSeconds(30) != 1800 || DurationSeconds(10080) != 604800) throw new InvalidOperationException("Duration conversion failed.");
        foreach (var value in new[] { double.NaN, 0, -1, 1.5, 10081 })
        {
            try { DurationSeconds(value); } catch (ArgumentException) { continue; }
            throw new InvalidOperationException("Invalid duration accepted.");
        }
        foreach (var offset in new[] { -1.0, 0.5 })
        {
            try { UntilSeconds(now.LocalDateTime.AddMinutes(offset), now); } catch (ArgumentException) { continue; }
            throw new InvalidOperationException("An end time in the past or under a minute away was accepted.");
        }
    }

    public async Task RenderVerificationAsync(string directory, JsonObject? snapshot)
    {
        try
        {
            AppWindow.Show(false);
            foreach (var theme in new[] { "light", "dark" })
                foreach (var view in new[] { "timerDuration", "timerTime", "awakeDuration", "awakeTime" })
                {
                    SetTheme(theme); Configure(view, snapshot); await Task.Delay(200);
                    // RenderTargetBitmap excludes the compositor's Mica backdrop.
                    root.Background = new SolidColorBrush(theme == "dark"
                        ? Windows.UI.Color.FromArgb(255, 32, 32, 32)
                        : Windows.UI.Color.FromArgb(255, 243, 243, 243));
                    await VisualVerification.SaveAsync(root, Path.Combine(directory, $"{view}-{theme}.png"));
                }
        }
        finally { root.Background = null; Hide(); }
    }
}
