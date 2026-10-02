using Microsoft.UI.Xaml.Controls;

namespace Doze.SettingsUi;

// Search finds settings inside pages, not only page names: every word must appear in a
// page's name or in the settings it contains.
public sealed partial class MainWindow
{
    private static readonly Dictionary<string, string> Keywords = new()
    {
        ["Overview"] = "status session control keep awake stop extend timer countdown final warning snooze stay audio playback start preset indefinitely",
        ["General"] = "launch sign in login startup start tray system tray notification area theme appearance dark light mode",
        ["Session defaults"] = "keep awake display screen turn off sleep default duration minutes power timer action hibernate shut down lock",
        ["After playback"] = "audio music video movie silence inactivity idle after playback one-shot action",
        ["Notifications"] = "final warning countdown length duration seconds notification preview snooze cancel",
        ["Agents"] = "mcp codex claude code agent lease heartbeat permissions skill connection keep alive connected sessions command line job",
        ["Advanced"] = "logging diagnostics log reset defaults data folder preferences file command line terminal powershell cli run watch job doze.exe",
        ["Menu guide"] = "menu guide help explain tray quick settings unavailable dimmed",
        ["About Doze"] = "version privacy about acknowledgements data folder clypy developer licenses"
    };

    internal static bool Matches(string page, string query) =>
        query.Split(' ', StringSplitOptions.RemoveEmptyEntries).All(word =>
            page.Contains(word, StringComparison.OrdinalIgnoreCase) || Keywords[page].Contains(word, StringComparison.OrdinalIgnoreCase));

    private static string[] Results(string query) => string.IsNullOrWhiteSpace(query) ? [] : Pages.Where(page => Matches(page, query)).ToArray();

    private void SearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput) return;
        var results = Results(sender.Text);
        sender.ItemsSource = results.Length > 0 || string.IsNullOrWhiteSpace(sender.Text) ? results : new[] { "No results" };
    }

    private void SearchChosen(AutoSuggestBox sender, AutoSuggestBoxSuggestionChosenEventArgs args)
    {
        if (Pages.Contains(args.SelectedItem as string)) SelectPage((string)args.SelectedItem);
    }

    private void SearchSubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        var match = args.ChosenSuggestion as string ?? Results(args.QueryText).FirstOrDefault();
        if (match is not null && Pages.Contains(match)) SelectPage(match);
    }

    private static void VerifySearch()
    {
        if (!Matches("Session defaults", "display sleep") || !Matches("General", "sign in") || !Matches("Advanced", "command line")
            || !Matches("Agents", "keep alive") || Matches("About Doze", "lease") || Results("display sleep").First() != "Session defaults")
            throw new InvalidOperationException("Search does not find settings by keyword.");
    }
}
