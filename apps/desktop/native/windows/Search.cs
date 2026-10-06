using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Doze.SettingsUi;

// Search finds individual settings: every card's title and description on every page is
// indexed, along with a few extra words people use for them. Choosing a result opens its
// page and scrolls to the setting.
public sealed partial class MainWindow
{
    private sealed record Entry(string Page, string Title, string Text)
    {
        public string Label => Title == Page ? Page : $"{Title} — {Page}";
    }

    private List<Entry>? index;
    /// Cards created while the index is being built.
    private static List<(string Title, string? Description, FrameworkElement? Control)>? collecting;

    private static readonly Dictionary<string, string> Keywords = new()
    {
        ["Overview"] = "status control start stop preset",
        ["General"] = "launch login startup tray icon flyout tooltip battery screen display",
        ["Session defaults"] = "default duration minutes snooze stay awake hibernate shut down lock",
        ["After playback"] = "audio music video movie silence inactivity idle",
        ["Notifications"] = "final warning countdown sound alert display monitor",
        ["Agents"] = "mcp codex claude code opencode gemini cursor hooks plugin lease approval allow deny",
        ["Advanced"] = "command line cli terminal powershell path doze run mcp server power request reset logs diagnostics",
        ["Menu guide"] = "help icon tray glyph sun",
        ["About Doze"] = "version privacy website"
    };

    /// Builds every page once, off screen, and records its card titles and descriptions.
    private List<Entry> Index()
    {
        if (index is not null) return index;
        var entries = new List<Entry>();
        var current = page;
        foreach (var name in Pages)
        {
            collecting = [];
            BeginPage(name);
            Build(name);
            entries.Add(new Entry(name, name, Keywords[name]));
            foreach (var (title, description, control) in collecting)
            {
                // A card's own buttons and choices are part of what people search for ("PATH").
                var controls = control is null ? "" : string.Join(" ",
                    new[] { (DependencyObject)control }.Concat(Descendants(control)).OfType<Control>().Select(ControlName).OfType<string>());
                entries.Add(new Entry(name, title, $"{description} {controls}"));
            }
            collecting = null;
        }
        page = current;
        ShowPage();
        return index = entries.DistinctBy(entry => entry.Label).ToList();
    }

    internal static bool Matches(string haystack, string query) =>
        query.Split(' ', StringSplitOptions.RemoveEmptyEntries).All(word => haystack.Contains(word, StringComparison.OrdinalIgnoreCase));

    private List<Entry> Results(string query) => string.IsNullOrWhiteSpace(query) ? [] : Index()
        .Where(entry => Matches($"{entry.Title} {entry.Text} {entry.Page}", query))
        // Settings whose title matches come before pages and descriptions.
        .OrderBy(entry => Matches(entry.Title, query) ? 0 : 1)
        .ThenBy(entry => entry.Title == entry.Page ? 1 : 0)
        .Take(12).ToList();

    private void SearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput) return;
        var results = Results(sender.Text).Select(entry => entry.Label).ToArray();
        sender.ItemsSource = results.Length > 0 || string.IsNullOrWhiteSpace(sender.Text) ? results : new[] { "No results" };
    }

    private void SearchChosen(AutoSuggestBox sender, AutoSuggestBoxSuggestionChosenEventArgs args)
    {
        if (Index().FirstOrDefault(entry => entry.Label == args.SelectedItem as string) is Entry entry) Reveal(entry);
    }

    private void SearchSubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        var entry = Index().FirstOrDefault(e => e.Label == args.ChosenSuggestion as string) ?? Results(args.QueryText).FirstOrDefault();
        if (entry is not null) Reveal(entry);
    }

    /// Opens the setting's page, scrolls it into view and moves focus to its control.
    private void Reveal(Entry entry)
    {
        SelectPage(entry.Page);
        if (entry.Title == entry.Page) return;
        Root.UpdateLayout();
        var title = Descendants(Cards).OfType<TextBlock>().FirstOrDefault(t => t.Text == entry.Title && t.Style == Style("CardTitleStyle"));
        if (title is null) return;
        title.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0.3, AnimationDesired = true });
        DependencyObject? card = title;
        while (card is not null && card is not Border and not Expander) card = Microsoft.UI.Xaml.Media.VisualTreeHelper.GetParent(card);
        if (card is not null && Descendants(card).OfType<Control>().FirstOrDefault(c => c.IsTabStop && c.IsEnabled) is Control control)
            control.Focus(FocusState.Programmatic);
    }

    private void VerifySearch()
    {
        foreach (var (query, page, title) in new[]
        {
            ("snooze", "Session defaults", "Snooze length"),
            ("battery", "General", "Stop keeping awake below"),
            ("checking in", "Agents", "If an agent stops checking in"),
            ("sound", "Notifications", "Play a sound when it appears"),
            ("path", "Advanced", "doze command-line tool"),
            ("cursor", "Agents", "Cursor"),
        })
        {
            var results = Results(query);
            if (!results.Any(entry => entry.Page == page && entry.Title == title))
                throw new InvalidOperationException($"Search for \"{query}\" did not find {title} ({string.Join(", ", results.Select(r => r.Label))}).");
        }
        if (Results("lease").Any(entry => entry.Page == "About Doze"))
            throw new InvalidOperationException("Search matched an unrelated page.");
    }
}
