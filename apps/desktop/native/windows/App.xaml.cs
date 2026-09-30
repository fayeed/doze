using Microsoft.UI.Xaml;
using System.Text.Json.Nodes;

namespace Doze.SettingsUi;

public partial class App : Application
{
    private MainWindow? window;
    private CountdownWindow? countdown;
    private EngineBridge? bridge;

    public App() => InitializeComponent();

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
                if (!commands.SequenceEqual(new[] { "cancel", "snooze" }))
                    throw new InvalidOperationException("Preview submitted an engine operation.");
                var arguments = Environment.GetCommandLineArgs();
                var render = Array.IndexOf(arguments, "--render-dir");
                if (render >= 0 && render + 1 < arguments.Length)
                {
                    await window.RenderVerificationAsync(arguments[render + 1]);
                    await countdown.RenderVerificationAsync(arguments[render + 1]);
                }
                await bridge.SendAsync("verified");
                Exit();
                return;
            }
            Receive(initial);
            var dispatcher = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            await bridge.ListenAsync(message => dispatcher.TryEnqueue(() => Receive(message)));
        }
        catch (Exception error)
        {
            await Console.Error.WriteLineAsync(error.ToString());
            Exit();
        }
    }

    private void Receive(JsonObject message)
    {
        var type = message["type"]?.GetValue<string>();
        if (type is "preview" or "countdown")
        {
            countdown ??= new CountdownWindow(command => bridge!.SendAsync(command));
            countdown.Receive(message);
        }
        else
        {
            if (type == "open") window ??= new MainWindow(bridge!, message);
            countdown?.Receive(message);
            if (message["command"]?.GetValue<string>() is not ("cancel" or "snooze"))
                window?.Receive(message);
        }
    }
}
