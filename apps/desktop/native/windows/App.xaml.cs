using Microsoft.UI.Xaml;
using System.Text.Json.Nodes;

namespace Doze.SettingsUi;

public partial class App : Application
{
    private MainWindow? window;
    private CountdownWindow? countdown;
    /// Copies of the final warning on the other displays, when Show on every display is on.
    private readonly List<CountdownWindow> mirrors = [];
    private TimerWindow? timer;
    private TrayFlyout? flyout;
    private EngineBridge? bridge;
    private string theme = "system";

    private void ChangeTheme(string value)
    {
        theme = value;
        window?.SetTheme(value);
        countdown?.SetTheme(value);
        foreach (var mirror in mirrors) mirror.SetTheme(value);
        timer?.SetTheme(value);
        flyout?.SetTheme(value);
    }

    public App()
    {
        InitializeComponent();
        // Report the cause instead of a bare stowed-exception crash code.
        UnhandledException += (_, args) => Console.Error.WriteLine($"Unhandled: {args.Message} {args.Exception}");
    }

    private void OpenTimer(JsonObject message)
    {
        timer ??= new TimerWindow(bridge!);
        timer.SetTheme(theme);
        timer.Open(message);
    }

    private void OpenTimerView(string view) => OpenTimer(new JsonObject
    {
        ["type"] = "open", ["view"] = view, ["snapshot"] = (window?.Snapshot ?? latest)?.DeepClone()
    });

    /// The latest snapshot from the engine, for windows opened from the flyout.
    private JsonObject? latest;

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            bridge = new EngineBridge();
            var initial = await bridge.ReadInitialAsync();
            if (Environment.GetCommandLineArgs().Contains("--verify-ui"))
            {
                window = new MainWindow(bridge, initial);
                window.VerifyPages();
                var commands = new List<string>();
                countdown = new CountdownWindow(command => { commands.Add(command); return Task.CompletedTask; }, true);
                await countdown.VerifyAsync();
                timer = new TimerWindow(bridge);
                timer.Verify(initial["snapshot"] as JsonObject);
                flyout = new TrayFlyout((_, _) => Task.CompletedTask, _ => { }, verification: true);
                flyout.Verify(initial["snapshot"]!.AsObject());
                if (!commands.SequenceEqual(new[] { "cancel", "snooze", "stay-awake" }))
                    throw new InvalidOperationException("Preview submitted an engine operation.");
                var arguments = Environment.GetCommandLineArgs();
                var render = Array.IndexOf(arguments, "--render-dir");
                if (render >= 0 && render + 1 < arguments.Length)
                {
                    await window.RenderVerificationAsync(arguments[render + 1]);
                    await countdown.RenderVerificationAsync(arguments[render + 1]);
                    await timer.RenderVerificationAsync(arguments[render + 1], initial["snapshot"] as JsonObject);
                    await flyout.RenderVerificationAsync(arguments[render + 1], initial["snapshot"]!.AsObject());
                }
                await bridge.SendAsync("verified");
                window.StopAppearance();
                countdown.StopAppearance();
                timer.StopAppearance();
                flyout.StopAppearance();
                timer.Close();
                flyout.Close();
                Exit();
                return;
            }
            Receive(initial);
            var dispatcher = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            await bridge.ListenAsync(message => dispatcher.TryEnqueue(() => Receive(message)));
            Stop();
            Exit();
        }
        catch (Exception error)
        {
            Stop();
            await Console.Error.WriteLineAsync(error.ToString());
            Exit();
        }
    }

    private void Stop()
    {
        window?.StopAppearance();
        countdown?.StopAppearance();
        foreach (var mirror in mirrors) mirror.StopAppearance();
        timer?.StopAppearance();
        flyout?.StopAppearance();
    }

    private void Receive(JsonObject message)
    {
        if (message["snapshot"] is JsonObject snapshot)
        {
            latest = snapshot;
            if (snapshot["settings"]?["theme"]?.GetValue<string>() is string saved && saved != theme) ChangeTheme(saved);
        }
        if (message["theme"]?.GetValue<string>() is string appearance && appearance != theme) ChangeTheme(appearance);
        var type = message["type"]?.GetValue<string>();
        if (type == "panel")
        {
            flyout ??= new TrayFlyout((command, fields) => bridge!.SendCommandAsync(command, fields), OpenTimerView);
            flyout.SetTheme(theme);
            flyout.Open(message);
            return;
        }
        if (type == "open" && message["view"]?.GetValue<string>() is "awakeDuration" or "awakeTime" or "timerDuration" or "timerTime")
        {
            OpenTimer(message);
            return;
        }
        timer?.Receive(message);
        flyout?.Receive(message);
        if (type is "preview" or "countdown")
        {
            countdown ??= new CountdownWindow(command => bridge!.SendAsync(command));
            countdown.SetTheme(theme);
            countdown.Receive(message);
            Mirror(message);
            return;
        }
        if (type == "open")
        {
            window ??= new MainWindow(bridge!, message, ChangeTheme, OpenTimerView);
            window.SetTheme(theme);
        }
        countdown?.Receive(message);
        window?.Receive(message);
    }

    /// Show on every display: one copy of the warning per other display. Each copy offers the
    /// same buttons, and every copy closes with the warning.
    private void Mirror(JsonObject message)
    {
        var showing = message["countdown"] is JsonObject || message["type"]?.GetValue<string>() == "preview";
        var displays = message["allDisplays"]?.GetValue<bool>() == true && showing
            ? Microsoft.UI.Windowing.DisplayArea.FindAll().ToArray()
            : [];
        var primary = countdown!.Display;
        var others = displays.Where(display => display.DisplayId.Value != primary).ToList();
        while (mirrors.Count > others.Count)
        {
            var extra = mirrors[^1];
            mirrors.RemoveAt(mirrors.Count - 1);
            extra.Receive(new JsonObject { ["type"] = "countdown", ["countdown"] = null });
            extra.StopAppearance();
        }
        for (var i = 0; i < others.Count; i++)
        {
            if (i == mirrors.Count) mirrors.Add(new CountdownWindow(command => bridge!.SendAsync(command), mirror: true));
            mirrors[i].SetTheme(theme);
            mirrors[i].PlaceOn(others[i]);
            mirrors[i].Receive(message);
        }
    }
}
