using Microsoft.UI.Xaml;
using System.Text.Json.Nodes;

namespace Doze.SettingsUi;

public partial class App : Application
{
    private MainWindow? window;
    private CountdownWindow? countdown;
    private TimerWindow? timer;
    private EngineBridge? bridge;
    private string theme = "system";

    private void ChangeTheme(string value)
    {
        theme = value;
        window?.SetTheme(value);
        countdown?.SetTheme(value);
        timer?.SetTheme(value);
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
                if (!commands.SequenceEqual(new[] { "cancel", "snooze", "stay-awake" }))
                    throw new InvalidOperationException("Preview submitted an engine operation.");
                var arguments = Environment.GetCommandLineArgs();
                var render = Array.IndexOf(arguments, "--render-dir");
                if (render >= 0 && render + 1 < arguments.Length)
                {
                    await window.RenderVerificationAsync(arguments[render + 1]);
                    await countdown.RenderVerificationAsync(arguments[render + 1]);
                    await timer.RenderVerificationAsync(arguments[render + 1], initial["snapshot"] as JsonObject);
                }
                await bridge.SendAsync("verified");
                window.StopAppearance();
                countdown.StopAppearance();
                timer.StopAppearance();
                timer.Close();
                Exit();
                return;
            }
            Receive(initial);
            var dispatcher = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            await bridge.ListenAsync(message => dispatcher.TryEnqueue(() => Receive(message)));
            window?.StopAppearance();
            countdown?.StopAppearance();
            timer?.StopAppearance();
            Exit();
        }
        catch (Exception error)
        {
            window?.StopAppearance();
            countdown?.StopAppearance();
            timer?.StopAppearance();
            await Console.Error.WriteLineAsync(error.ToString());
            Exit();
        }
    }

    private void Receive(JsonObject message)
    {
        if (message["theme"]?.GetValue<string>() is string appearance)
            ChangeTheme(appearance);
        else if (message["type"]?.GetValue<string>() == "open"
                 && message["snapshot"]?["settings"]?["theme"]?.GetValue<string>() is string savedTheme)
            ChangeTheme(savedTheme);
        var type = message["type"]?.GetValue<string>();
        if (type == "open" && message["view"]?.GetValue<string>() is "awakeDuration" or "awakeTime" or "timerDuration" or "timerTime")
        {
            OpenTimer(message);
            return;
        }
        timer?.Receive(message);
        if (type is "preview" or "countdown")
        {
            countdown ??= new CountdownWindow(command => bridge!.SendAsync(command));
            countdown.SetTheme(theme);
            countdown.Receive(message);
        }
        else
        {
            if (type == "open")
            {
                window ??= new MainWindow(bridge!, message, ChangeTheme, view => OpenTimer(new JsonObject
                {
                    ["type"] = "open", ["view"] = view, ["snapshot"] = window?.Snapshot.DeepClone()
                }));
                window.SetTheme(theme);
            }
            countdown?.Receive(message);
            window?.Receive(message);
        }
    }
}
