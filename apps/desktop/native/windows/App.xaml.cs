using Microsoft.UI.Xaml;

namespace Doze.SettingsUi;

public partial class App : Application
{
    private MainWindow? window;

    public App() => InitializeComponent();

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            var bridge = new EngineBridge();
            var initial = await bridge.ReadInitialAsync();
            window = new MainWindow(bridge, initial);
            if (Environment.GetCommandLineArgs().Contains("--verify-ui"))
            {
                window.VerifyPages();
                var arguments = Environment.GetCommandLineArgs();
                var render = Array.IndexOf(arguments, "--render-dir");
                if (render >= 0 && render + 1 < arguments.Length)
                    await window.RenderVerificationAsync(arguments[render + 1]);
                await bridge.SendAsync("verified");
                Exit();
                return;
            }
            window.Activate();
            await bridge.ListenAsync(message => window.Receive(message));
        }
        catch (Exception error)
        {
            await Console.Error.WriteLineAsync(error.ToString());
            Exit();
        }
    }
}
