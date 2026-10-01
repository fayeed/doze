using Microsoft.UI.Xaml.Controls;

namespace Doze.SettingsUi;

public sealed partial class MainWindow
{
    private void SearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason == AutoSuggestionBoxTextChangeReason.UserInput)
            sender.ItemsSource = Pages.Where(name => name.Contains(sender.Text, StringComparison.OrdinalIgnoreCase)).ToArray();
    }

    private void SearchChosen(AutoSuggestBox sender, AutoSuggestBoxSuggestionChosenEventArgs args) => SelectPage((string)args.SelectedItem);

    private void SearchSubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        var match = Pages.FirstOrDefault(name => name.Contains(args.QueryText, StringComparison.OrdinalIgnoreCase));
        if (match is not null) SelectPage(match);
    }
}
