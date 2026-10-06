using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json.Nodes;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using Windows.System;

namespace Doze.SettingsUi;

public sealed partial class CountdownWindow : Window
{
    private readonly Func<string, Task> send;
    private readonly bool verification;
    private readonly DispatcherTimer clock = new() { Interval = TimeSpan.FromSeconds(1) };
    private readonly Stopwatch previewClock = new();
    private bool preview;
    private bool visible;
    private bool pending;
    private string action = "Sleep";
    /// A copy on another display (Show on every display): silent, and placed by the app.
    private readonly bool mirror;

    public CountdownWindow(Func<string, Task> send, bool verification = false, bool mirror = false)
    {
        this.send = send;
        this.verification = verification;
        this.mirror = mirror;
        InitializeComponent();
        WindowAppearance.Observe(this, Root);
        Title = "Doze · Countdown";
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(TitleBar);
        SystemBackdrop = new MicaBackdrop { Kind = Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt };
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.ico"));
        var titleIcon = Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.png");
        if (File.Exists(titleIcon)) TitleIcon.Source = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri(titleIcon)) { DecodePixelWidth = 32 };
        AppWindow.IsShownInSwitchers = true;
        ResizeWarning();
        Notice.RegisterPropertyChangedCallback(InfoBar.IsOpenProperty, (_, _) =>
        {
            // Keep the action and remaining time visible when an error occupies
            // the footer; closing the message restores the compact warning.
            ResizeWarning();
        });
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.IsAlwaysOnTop = true;
            presenter.IsResizable = false;
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
        }
        AppWindow.Closing += (window, args) =>
        {
            args.Cancel = true;
            _ = CommandAsync("cancel");
        };
        clock.Tick += (_, _) =>
        {
            var remaining = Math.Max(0, 60 - (int)previewClock.Elapsed.TotalSeconds);
            UpdateTime((ulong)remaining);
            if (remaining == 0) clock.Stop();
        };
    }

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(nint window);

    private void ResizeWarning()
    {
        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var width = Math.Min((int)(560 * scale), (int)(area.Width * 0.92));
        var height = Math.Min((int)((Notice.IsOpen ? 540 : 420) * scale), (int)(area.Height * 0.92));
        AppWindow.MoveAndResize(new RectInt32(area.X + (area.Width - width) / 2, area.Y + (area.Height - height) / 2, width, height));
    }

    /// The display the warning is on.
    public ulong Display => DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).DisplayId.Value;

    /// Centers the warning on another display.
    public void PlaceOn(DisplayArea display)
    {
        var area = display.WorkArea;
        var size = AppWindow.Size;
        AppWindow.Move(new PointInt32(area.X + (area.Width - size.Width) / 2, area.Y + (area.Height - size.Height) / 2));
    }

    [DllImport("winmm.dll", CharSet = CharSet.Unicode)]
    private static extern bool PlaySound(string sound, nint module, uint flags);

    /// The Windows notification sound, once, when a real warning appears.
    private static void Chime() => PlaySound("SystemNotification", 0, 0x00010000 | 0x0001 | 0x0002); // SND_ALIAS | SND_ASYNC | SND_NODEFAULT

    public void SetTheme(string value) => WindowAppearance.Apply(Root, value);
    public void StopAppearance() => WindowAppearance.Stop(Root);

    public void Receive(JsonObject message)
    {
        var type = message["type"]?.GetValue<string>();
        if (message["snoozeMinutes"]?.GetValue<int>() is int snooze)
        {
            SnoozeButton.Content = $"Snooze {snooze} minutes";
            Description.Text = $"Cancel the action or snooze for {snooze} minutes.";
        }
        if (type == "preview")
        {
            if (visible && !preview) return;
            preview = true;
            action = message["action"]?.GetValue<string>() ?? "Sleep";
            previewClock.Restart();
            clock.Start();
            UpdateTime(60);
            ShowWarning();
        }
        else if (type == "countdown")
        {
            if (message["countdown"] is JsonObject countdown)
            {
                var newlyShown = !visible || preview;
                preview = false;
                clock.Stop();
                action = countdown["action"]!.GetValue<string>();
                UpdateTime(countdown["remaining"]!.GetValue<ulong>());
                ShowWarning(newlyShown);
                if (newlyShown && !mirror && !verification && message["sound"]?.GetValue<bool>() != false) Chime();
            }
            else if (!preview) HideWarning();
        }
        else if (message["command"]?.GetValue<string>() is "cancel" or "snooze" or "stay-awake")
        {
            SetPending(false);
            if (message["error"] is JsonValue error)
            {
                Notice.Title = "Couldn't update countdown";
                Notice.Message = error.GetValue<string>();
                Notice.Severity = InfoBarSeverity.Error;
                Notice.IsOpen = true;
            }
        }
    }

    private void UpdateTime(ulong seconds)
    {
        ActionText.Text = $"{action} in";
        TimeText.Text = seconds >= 3600
            ? $"{seconds / 3600}:{seconds / 60 % 60:00}:{seconds % 60:00}"
            : $"{seconds / 60:00}:{seconds % 60:00}";
        AutomationProperties.SetName(TimeText, $"{action} in {seconds / 60} minutes and {seconds % 60} seconds");
        PreviewHint.Visibility = preview ? Visibility.Visible : Visibility.Collapsed;
        StayAwakeButton.Visibility = preview ? Visibility.Collapsed : Visibility.Visible;
        CancelButton.Content = preview ? "Dismiss preview" : "Cancel action";
    }

    private void ShowWarning(bool reset = true)
    {
        // Engine ticks can arrive before a command's acknowledgement. Keep the
        // command disabled and its error visible while refreshing the same warning.
        if (reset)
        {
            Notice.IsOpen = false;
            ContentScroller.ChangeView(null, 0, null, true);
            SetPending(false);
        }
        // Appear on top without taking keyboard focus: typing elsewhere must never press
        // Snooze, Cancel or Stay Awake. Escape works once the warning is clicked.
        if (!visible && !verification) AppWindow.Show(false);
        visible = true;
    }

    private void HideWarning()
    {
        clock.Stop();
        previewClock.Stop();
        visible = false;
        preview = false;
        SetPending(false);
        AppWindow.Hide();
    }

    private void SetPending(bool value)
    {
        pending = value;
        CancelButton.IsEnabled = SnoozeButton.IsEnabled = StayAwakeButton.IsEnabled = !value;
    }

    private async Task CommandAsync(string command)
    {
        if (!visible || pending) return;
        if (preview)
        {
            HideWarning();
            return;
        }
        Notice.IsOpen = false;
        SetPending(true);
        try { await send(command); }
        catch (Exception error)
        {
            SetPending(false);
            Notice.Title = "Couldn't update countdown";
            Notice.Message = error.Message;
            Notice.Severity = InfoBarSeverity.Error;
            Notice.IsOpen = true;
        }
    }

    private async void Cancel(object sender, RoutedEventArgs args) => await CommandAsync("cancel");
    private async void StayAwake(object sender, RoutedEventArgs args) => await CommandAsync("stay-awake");
    private async void Snooze(object sender, RoutedEventArgs args) => await CommandAsync("snooze");
    private async void KeyPressed(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key != VirtualKey.Escape) return;
        args.Handled = true;
        await CommandAsync("cancel");
    }

    public async Task VerifyAsync()
    {
        Receive(new JsonObject { ["type"] = "preview", ["action"] = "Sleep" });
        if (TimeText.Text != "01:00" || TimeText.FontSize < 64 || !preview)
            throw new InvalidOperationException("Preview countdown is not legible.");
        if (AppWindow.Presenter is not OverlappedPresenter { IsAlwaysOnTop: true })
            throw new InvalidOperationException("Preview is not always on top.");
        await CommandAsync("snooze");
        if (visible) throw new InvalidOperationException("Preview snooze did not dismiss the window.");
        Receive(new JsonObject { ["type"] = "countdown", ["countdown"] = new JsonObject { ["action"] = "Sleep", ["remaining"] = 300UL } });
        Receive(new JsonObject { ["type"] = "preview", ["action"] = "Shut down" });
        if (preview || ActionText.Text != "Sleep in" || TimeText.Text != "05:00")
            throw new InvalidOperationException("Preview replaced a real countdown.");
        if (AppWindow.Presenter is not OverlappedPresenter { IsAlwaysOnTop: true })
            throw new InvalidOperationException("Countdown is not always on top.");
        await CommandAsync("cancel");
        Receive(new JsonObject { ["type"] = "countdown", ["countdown"] = new JsonObject { ["action"] = "Sleep", ["remaining"] = 299UL } });
        if (!pending || CancelButton.IsEnabled || SnoozeButton.IsEnabled || StayAwakeButton.IsEnabled)
            throw new InvalidOperationException("Countdown update enabled duplicate commands before acknowledgement.");
        await CommandAsync("snooze");
        Receive(new JsonObject { ["type"] = "countdown" });
        if (visible) throw new InvalidOperationException("Cancelled countdown did not hide.");
        Receive(new JsonObject { ["type"] = "countdown", ["countdown"] = new JsonObject { ["action"] = "Sleep", ["remaining"] = 300UL } });
        await CommandAsync("snooze");
        Receive(new JsonObject { ["type"] = "countdown" });
        if (visible) throw new InvalidOperationException("Snoozed countdown did not hide.");
        Receive(new JsonObject { ["type"] = "countdown", ["countdown"] = new JsonObject { ["action"] = "Sleep", ["remaining"] = 300UL } });
        if (StayAwakeButton.Visibility != Visibility.Visible) throw new InvalidOperationException("Stay Awake is missing from the real countdown.");
        await CommandAsync("stay-awake");
        Receive(new JsonObject { ["type"] = "error", ["command"] = "stay-awake", ["error"] = "Test engine refusal" });
        if (pending || !Notice.IsOpen || Notice.Message != "Test engine refusal")
            throw new InvalidOperationException("Stay Awake failure was not shown or retry was blocked.");
        Receive(new JsonObject { ["type"] = "countdown", ["countdown"] = new JsonObject { ["action"] = "Sleep", ["remaining"] = 299UL } });
        if (!Notice.IsOpen)
            throw new InvalidOperationException("Countdown update erased the command failure.");
        Receive(new JsonObject { ["type"] = "countdown" });
        if (visible) throw new InvalidOperationException("Stay Awake did not dismiss the countdown.");
    }

    public async Task RenderVerificationAsync(string directory)
    {
        try
        {
            foreach (var theme in new[] { ElementTheme.Light, ElementTheme.Dark })
            {
                Root.RequestedTheme = theme;
                Root.Background = new SolidColorBrush(theme == ElementTheme.Dark
                    ? Windows.UI.Color.FromArgb(255, 32, 32, 32)
                    : Windows.UI.Color.FromArgb(255, 243, 243, 243));
                Receive(new JsonObject { ["type"] = "preview", ["action"] = "Sleep" });
                AppWindow.Show(false);
                await Task.Delay(150);
                await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"Countdown-{theme}.png"));
                HideWarning();
                Receive(new JsonObject { ["type"] = "countdown", ["countdown"] = new JsonObject { ["action"] = "Sleep", ["remaining"] = 300UL } });
                AppWindow.Show(false);
                await Task.Delay(150);
                await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"Countdown-real-{theme}.png"));
                Receive(new JsonObject { ["type"] = "error", ["command"] = "stay-awake", ["error"] = "Windows could not update the power request." });
                await Task.Delay(150);
                Root.UpdateLayout();
                var content = (FrameworkElement)((FrameworkElement)ActionText.Parent).Parent;
                if (ActionText.TransformToVisual(content).TransformPoint(new Windows.Foundation.Point(0, 0)).Y < 0)
                    throw new InvalidOperationException("Countdown action is clipped by the error message.");
                await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"Countdown-error-{theme}.png"));
                // A 768-pixel display at 200% scaling leaves less than 384 DIPs
                // for the warning. Verify that its essential content remains reachable.
                var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
                AppWindow.Resize(new SizeInt32((int)(560 * scale), (int)(360 * scale)));
                await Task.Delay(150);
                Root.UpdateLayout();
                await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"Countdown-constrained-{theme}.png"));
                if (ActionText.TransformToVisual(content).TransformPoint(new Windows.Foundation.Point(0, 0)).Y < 0)
                    throw new InvalidOperationException("Countdown action is clipped on a constrained display.");
                if (ContentScroller.ScrollableHeight <= 0)
                    throw new InvalidOperationException("Constrained countdown cannot scroll to its controls.");
                ContentScroller.ChangeView(null, ContentScroller.ScrollableHeight, null, true);
                await Task.Delay(150);
                var bottom = StayAwakeButton.TransformToVisual(ContentScroller).TransformPoint(new Windows.Foundation.Point(0, StayAwakeButton.ActualHeight)).Y;
                if (bottom > ContentScroller.ActualHeight || bottom < 0)
                    throw new InvalidOperationException("Constrained countdown controls are unreachable.");
                await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"Countdown-constrained-controls-{theme}.png"));
                HideWarning();
            }
        }
        finally { Root.Background = null; HideWarning(); }
    }
}
