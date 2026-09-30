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

    public CountdownWindow(Func<string, Task> send, bool verification = false)
    {
        this.send = send;
        this.verification = verification;
        InitializeComponent();
        Title = "Doze · Countdown";
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(TitleBar);
        SystemBackdrop = new MicaBackdrop { Kind = Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt };
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.ico"));
        AppWindow.IsShownInSwitchers = true;
        var dpi = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        AppWindow.Resize(new SizeInt32((int)(560 * dpi), (int)(420 * dpi)));
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

    public void Receive(JsonObject message)
    {
        var type = message["type"]?.GetValue<string>();
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
                preview = false;
                clock.Stop();
                action = countdown["action"]!.GetValue<string>();
                UpdateTime(countdown["remaining"]!.GetValue<ulong>());
                ShowWarning();
            }
            else if (!preview) HideWarning();
        }
        else if (message["command"]?.GetValue<string>() is "cancel" or "snooze")
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
        CancelButton.Content = preview ? "Dismiss preview" : "Cancel action";
    }

    private void ShowWarning()
    {
        Notice.IsOpen = false;
        SetPending(false);
        if (!visible && !verification) Activate();
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
        CancelButton.IsEnabled = SnoozeButton.IsEnabled = !value;
    }

    private async Task CommandAsync(string command)
    {
        if (!visible || pending) return;
        if (preview)
        {
            HideWarning();
            return;
        }
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
        Receive(new JsonObject { ["type"] = "countdown" });
        if (visible) throw new InvalidOperationException("Cancelled countdown did not hide.");
        Receive(new JsonObject { ["type"] = "countdown", ["countdown"] = new JsonObject { ["action"] = "Sleep", ["remaining"] = 300UL } });
        await CommandAsync("snooze");
        Receive(new JsonObject { ["type"] = "countdown" });
        if (visible) throw new InvalidOperationException("Snoozed countdown did not hide.");
    }

    public async Task RenderVerificationAsync(string directory)
    {
        try
        {
            foreach (var theme in new[] { ElementTheme.Light, ElementTheme.Dark })
            {
                Root.RequestedTheme = theme;
                Receive(new JsonObject { ["type"] = "preview", ["action"] = "Sleep" });
                AppWindow.Show(false);
                await Task.Delay(150);
                await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"Countdown-{theme}.png"));
                HideWarning();
            }
        }
        finally { HideWarning(); }
    }
}
