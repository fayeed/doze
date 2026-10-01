using System.Text.Json.Nodes;

namespace Doze.SettingsUi;

public sealed partial class MainWindow
{
    private Microsoft.UI.Xaml.Controls.TextBlock? overviewStatus;
    private Microsoft.UI.Xaml.Controls.TextBlock? overviewTimer;

    private bool HasDeadline => false;

    private static string OverviewShape(JsonObject snapshot) => "";

    private void Overview()
    {
        overviewStatus = Card("Current session", snapshot["status"]?.GetValue<string>() ?? "Normal sleep allowed", "");
        overviewTimer = Card("Power action", snapshot["timerStatus"]?.GetValue<string>() ?? "No power action scheduled", "");
        Section("Related");
        Card("Preview the final warning", "Try Cancel and Snooze without scheduling a power action.", "", ActionButton("Preview", () => Send("preview")));
        Card("Data folder", "Your preferences and optional diagnostic logs.", "", ActionButton("Open folder", OpenData));
    }

    private void UpdateOverview()
    {
        if (overviewStatus is not null) overviewStatus.Text = snapshot["status"]?.GetValue<string>() ?? "";
        if (overviewTimer is not null) overviewTimer.Text = snapshot["timerStatus"]?.GetValue<string>() ?? "";
    }
}
