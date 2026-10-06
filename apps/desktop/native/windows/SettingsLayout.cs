using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media.Imaging;

namespace Doze.SettingsUi;

// The Windows 11 Settings layout: section headers over groups of cards spaced 4 px apart.
// Each card has an icon, a title with its description underneath, and its control on the
// right; narrow windows move the control under the text instead of squeezing it.
public sealed partial class MainWindow
{
    private Panel group = null!;
    private readonly Dictionary<string, bool> expanded = new();

    private static Style Style(string key) => (Style)Application.Current.Resources[key];

    private void BeginPage(string title)
    {
        Cards.Children.Clear();
        PageTitle.Text = title;
        group = NewGroup();
    }

    private StackPanel NewGroup()
    {
        var panel = new StackPanel { Spacing = 4 };
        Cards.Children.Add(panel);
        return panel;
    }

    private void Section(string title)
    {
        var header = new TextBlock { Text = title, Style = Style("SectionHeaderStyle") };
        AutomationProperties.SetHeadingLevel(header, AutomationHeadingLevel.Level2);
        // A page that starts with a section needs no gap under the title.
        if (Cards.Children.Count == 1 && group.Children.Count == 0)
        {
            Cards.Children.Clear();
            header.Margin = new Thickness(1, 0, 0, 8);
        }
        Cards.Children.Add(header);
        group = NewGroup();
    }

    private TextBlock Footer(string text)
    {
        var footer = new TextBlock { Text = text, Style = Style("FooterTextStyle") };
        group.Children.Add(footer);
        return footer;
    }

    /// Adds a settings card and returns its description, which live pages update in place.
    private TextBlock Card(string title, string? description, object? icon, FrameworkElement? control = null)
    {
        var content = CardGrid(title, description, icon, control, out var text);
        group.Children.Add(new Border { Style = Style("SettingsCardStyle"), Child = content });
        return text;
    }

    /// A card whose sub-options appear in rows underneath when expanded, like Windows Settings.
    private StackPanel ExpanderCard(string title, string? description, object? icon, FrameworkElement? control = null, bool open = true)
    {
        var header = CardGrid(title, description, icon, control, out _);
        header.Margin = new Thickness(0, 12, 0, 12);
        var rows = new StackPanel();
        var expander = new Expander
        {
            Style = Style("SettingsExpanderStyle"), Header = header, Content = rows,
            IsExpanded = expanded.GetValueOrDefault(title, open)
        };
        // Distinct from the header's own control, which already carries the title.
        AutomationProperties.SetName(expander, control is null ? title : "More options for " + title);
        expander.Expanding += (_, _) => expanded[title] = true;
        expander.Collapsed += (_, _) => expanded[title] = false;
        group.Children.Add(expander);
        return rows;
    }

    private TextBlock Row(StackPanel rows, string title, string? description, FrameworkElement? control = null)
    {
        var content = CardGrid(title, description, null, control, out var text);
        rows.Children.Add(new Border { Style = Style("SettingsRowStyle"), Child = content });
        return text;
    }

    private static Grid CardGrid(string title, string? description, object? icon, FrameworkElement? control, out TextBlock descriptionText)
    {
        collecting?.Add((title, description, control));
        var grid = new Grid { VerticalAlignment = VerticalAlignment.Center };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        if (Icon(icon) is FrameworkElement image)
        {
            image.Margin = new Thickness(2, 0, 20, 0);
            image.VerticalAlignment = VerticalAlignment.Center;
            AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
            Grid.SetRowSpan(image, 2);
            grid.Children.Add(image);
        }
        var text = new StackPanel { Spacing = 1, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = title, Style = Style("CardTitleStyle") });
        descriptionText = new TextBlock
        {
            Text = description ?? "", Style = Style("CardDescriptionStyle"),
            Visibility = string.IsNullOrEmpty(description) ? Visibility.Collapsed : Visibility.Visible
        };
        text.Children.Add(descriptionText);
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);
        if (control is null) return grid;
        if (control is Control named && string.IsNullOrEmpty(AutomationProperties.GetName(named)) && named is not Button)
            AutomationProperties.SetName(named, title);
        control.VerticalAlignment = VerticalAlignment.Center;
        grid.Children.Add(control);
        // The control's natural width is read once, while it sits in the auto-sized column.
        // Re-measuring here, or re-placing on height changes, would make layout cycle.
        double? natural = null;
        bool? placedBelow = null;
        void Arrange(double width)
        {
            natural ??= control.DesiredSize.Width;
            var below = width < 440 || natural > width * 0.55;
            if (below == placedBelow) return;
            placedBelow = below;
            Grid.SetRow(control, below ? 1 : 0);
            Grid.SetColumn(control, below ? 1 : 2);
            control.HorizontalAlignment = below ? HorizontalAlignment.Left : HorizontalAlignment.Right;
            control.Margin = below ? new Thickness(0, 10, 0, 0) : new Thickness(16, 0, 0, 0);
        }
        Grid.SetColumn(control, 2);
        control.HorizontalAlignment = HorizontalAlignment.Right;
        control.Margin = new Thickness(16, 0, 0, 0);
        grid.SizeChanged += (_, args) => { if (args.NewSize.Width != args.PreviousSize.Width) Arrange(args.NewSize.Width); };
        return grid;
    }

    /// A brand tray glyph for the Menu guide and the status card.
    private static FrameworkElement Glyph(string state, double size)
    {
        var glyph = Glyphs.Create(state, size, state == "normal" ? "{ThemeResource TextFillColorSecondaryBrush}" : "{ThemeResource DozeTrayAccentBrush}");
        glyph.HorizontalAlignment = HorizontalAlignment.Center;
        glyph.VerticalAlignment = VerticalAlignment.Center;
        return glyph;
    }

    private static FrameworkElement? Icon(object? icon) => icon switch
    {
        string glyph => new FontIcon { Glyph = glyph, FontSize = 20 },
        FrameworkElement element => element,
        _ => null
    };

    /// A coloured Fluent icon whose colour follows this window's theme.
    private static FontIcon Tinted(string glyph, string brush, double size = 20) =>
        (FontIcon)XamlReader.Load($"<FontIcon xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Glyph=\"&#x{(int)glyph[0]:X4};\" FontSize=\"{size}\" Foreground=\"{{ThemeResource {brush}}}\" />");

    /// A bundled image from the Assets folder beside the companion, never fetched from the network.
    private static FrameworkElement Asset(string file, double size, string fallbackGlyph)
    {
        var path = Path.Combine(AppContext.BaseDirectory, "Assets", file);
        if (!File.Exists(path)) return new FontIcon { Glyph = fallbackGlyph, FontSize = size * 0.6, Width = size, Height = size };
        return new Image
        {
            Source = new BitmapImage(new Uri(path)) { DecodePixelWidth = (int)(size * 2), DecodePixelType = DecodePixelType.Logical },
            Width = size, Height = size
        };
    }

    /// An On/Off switch labelled like Windows Settings, with its state written to the left.
    private FrameworkElement Switch(string name, bool value, Action<bool> apply, bool enabled = true)
    {
        var state = new TextBlock { Text = value ? "On" : "Off", VerticalAlignment = VerticalAlignment.Center, Opacity = enabled ? 1 : 0.5 };
        AutomationProperties.SetAccessibilityView(state, AccessibilityView.Raw);
        var toggle = new ToggleSwitch { IsOn = value, IsEnabled = enabled, OnContent = "", OffContent = "", MinWidth = 0, Margin = new Thickness(0, 0, -12, 0) };
        AutomationProperties.SetName(toggle, name);
        toggle.Toggled += (_, _) => { state.Text = toggle.IsOn ? "On" : "Off"; apply(toggle.IsOn); };
        var panel = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        panel.Children.Add(state);
        panel.Children.Add(toggle);
        return panel;
    }

    private ComboBox Choice<T>(string name, IEnumerable<(T Value, string Label)> options, T selected, Action<T> apply, bool enabled = true)
    {
        var combo = new ComboBox { MinWidth = 200, IsEnabled = enabled };
        AutomationProperties.SetName(combo, name);
        foreach (var (value, label) in options) combo.Items.Add(new ComboBoxItem { Content = label, Tag = value });
        combo.SelectedItem = combo.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, selected));
        combo.SelectionChanged += (_, _) =>
        {
            if (combo.SelectedItem is ComboBoxItem { Tag: T value } && !Equals(value, selected)) { selected = value; apply(value); }
        };
        return combo;
    }

    /// Friendly durations; a saved value outside the list stays selectable.
    private static IEnumerable<(int, string)> MinuteChoices(int current) =>
        new[] { 5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480, 720, 1440, current }
            .Distinct().Order().Select(minutes => (minutes, Labels.Minutes(minutes)));

    private static IEnumerable<(int, string)> SecondChoices(int current, params int[] choices) =>
        choices.Append(current).Distinct().Order().Select(seconds => (seconds, Labels.Seconds(seconds)));

    private IEnumerable<(string, string)> ActionChoices => Actions.Select(action => (action, Labels.Action(action)));

    private Button ActionButton(string label, Func<Task> action, string? accessibleName = null, bool accent = false)
    {
        var button = new Button { Content = label };
        if (accent) button.Style = Style("AccentButtonStyle");
        if (accessibleName is not null) AutomationProperties.SetName(button, accessibleName);
        button.Click += async (_, _) =>
        {
            try { await action(); }
            catch (Exception error) { Notify("Couldn't complete that", error.Message, InfoBarSeverity.Error); }
        };
        return button;
    }

    private static WrapPanel Buttons(params FrameworkElement[] buttons)
    {
        var panel = new WrapPanel();
        foreach (var button in buttons) panel.Children.Add(button);
        return panel;
    }
}
