using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Doze.SettingsUi;

public sealed partial class MainWindow
{
    private ContentDialog? agentDialog;
    private string? agentDialogName;
    private bool agentDialogPermissions;
    private string? agentDialogFingerprint;

    private async Task OpenAgentSetupAsync(string name) => await OpenAgentDialogAsync(name, false);
    private async Task OpenAgentPermissionsAsync(string name) => await OpenAgentDialogAsync(name, true);

    private async Task OpenAgentDialogAsync(string name, bool permissions)
    {
        if (agentDialog is not null) return;
        agentDialogName = name;
        agentDialogPermissions = permissions;
        agentDialogFingerprint = null;
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            RequestedTheme = Root.RequestedTheme,
            Title = permissions ? $"{name} permissions" : $"Set up {name}",
            CloseButtonText = "Done",
            SecondaryButtonText = permissions ? "Revoke connection" : "",
            DefaultButton = ContentDialogButton.Close
        };
        agentDialog = dialog;
        RefreshAgentDialog();
        try
        {
            var shown = dialog.ShowAsync();
            if (!permissions && AgentClient(name) is null)
                await bridge.SendAgentAsync("agent-connect", name: name);
            var result = await shown;
            var id = AgentClient(name)?["id"]?.GetValue<string>();
            agentDialog = null;
            if (result == ContentDialogResult.Secondary && id is not null)
            {
                var confirm = new ContentDialog
                {
                    XamlRoot = Root.XamlRoot, RequestedTheme = Root.RequestedTheme,
                    Title = $"Revoke {name}?",
                    Content = "This cancels its active sessions and invalidates its credential. You will need to configure the client again.",
                    PrimaryButtonText = "Revoke", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Close
                };
                if (await confirm.ShowAsync() == ContentDialogResult.Primary)
                    await bridge.SendAgentAsync("agent-revoke", id: id);
            }
        }
        finally { dialog.Hide(); agentDialog = null; agentDialogName = null; agentDialogFingerprint = null; }
    }

    private JsonObject? AgentClient(string name) => (snapshot["settings"]?["agents"]?["clients"] as JsonArray)?
        .OfType<JsonObject>().FirstOrDefault(c => c["name"]?.GetValue<string>() == name);

    private void RefreshAgentDialog()
    {
        if (agentDialog is null || agentDialogName is null) return;
        var client = AgentClient(agentDialogName);
        if (agentDialogPermissions && client is null) { agentDialog.Hide(); return; }
        var connection = (snapshot["agentConnections"] as JsonArray)?.OfType<JsonObject>()
            .FirstOrDefault(c => c["name"]?.GetValue<string>() == agentDialogName);
        var fingerprint = agentDialogPermissions ? client?.ToJsonString() : connection?.ToJsonString() + snapshot["agentSkills"]?.ToJsonString() + snapshot["agentSkillMessage"]?.ToJsonString();
        if (fingerprint is not null && fingerprint == agentDialogFingerprint) return;
        agentDialogFingerprint = fingerprint;
        agentDialog.Content = agentDialogPermissions && client is not null
            ? AgentPermissionContent(client)
            : AgentSetupContent(agentDialogName, connection);
    }

    private StackPanel AgentSetupContent(string name, JsonObject? connection)
    {
        var content = new StackPanel { Spacing = 16, Width = 480, MaxWidth = 480 };
        content.Children.Add(new TextBlock
        {
            Text = "Add Doze to your client, then reload it. Creating a profile here does not install or verify the client connection.",
            TextWrapping = TextWrapping.Wrap
        });
        if (connection is null)
        {
            content.Children.Add(new ProgressRing { IsActive = true, Width = 28, Height = 28 });
            content.Children.Add(new TextBlock { Text = "Creating your local profile…" });
            return content;
        }
        content.Children.Add(new TextBlock
        {
            Text = name == "Codex" ? "1. Add this to ~/.codex/config.toml."
                : name == "Claude Code" ? "1. Add this JSON with claude mcp add-json --scope user doze '<JSON>'."
                : "1. Add this to your client’s mcpServers configuration.",
            TextWrapping = TextWrapping.Wrap
        });
        var config = connection[name == "Codex" ? "codex" : name == "Claude Code" ? "claude" : "generic"]!.GetValue<string>();
        var text = new TextBox
        {
            Text = config, IsReadOnly = true, AcceptsReturn = true, TextWrapping = TextWrapping.NoWrap,
            Height = 190, FontFamily = new FontFamily("Consolas")
        };
        ScrollViewer.SetHorizontalScrollBarVisibility(text, ScrollBarVisibility.Auto);
        AutomationProperties.SetName(text, $"{name} connection configuration");
        content.Children.Add(text);
        var copy = new Button { Content = "Copy configuration" };
        copy.Click += (_, _) =>
        {
            try
            {
                var data = new Windows.ApplicationModel.DataTransfer.DataPackage();
                data.SetText(config);
                Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(data);
                copy.Content = "Copied";
            }
            catch (Exception error) { Notify("Couldn't copy configuration", error.Message, InfoBarSeverity.Error); }
        };
        content.Children.Add(copy);
        AddAgentSkillControls(content, name);
        content.Children.Add(new TextBlock { Text = "2. Reload your client and ask it to use Doze.\n3. Approve its first request in Doze, or choose persistent permissions in Settings.", TextWrapping = TextWrapping.Wrap });
        content.Children.Add(new TextBlock { Text = "This configuration contains a private credential. Keep it out of shared files and source control.", Opacity = 0.7, FontSize = 12, TextWrapping = TextWrapping.Wrap });
        return content;
    }

    private void AddAgentSkillControls(StackPanel content, string name)
    {
        content.Children.Add(new TextBlock { Text = "Companion skill", FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
        var skill = (snapshot["agentSkills"] as JsonArray)?.OfType<JsonObject>().FirstOrDefault(s => s["name"]?.GetValue<string>() == name);
        var status = skill?["status"]?.GetValue<string>() ?? "manual";
        var path = skill?["path"]?.GetValue<string>();
        var description = status switch
        {
            "installed" => "Installed · " + path,
            "update_available" => "An existing copy differs from this release. Update only after reviewing your local edits.",
            "blocked" => skill?["error"]?.GetValue<string>() ?? "Automatic installation is unavailable for this folder. Copy the skill manually.",
            "not_installed" => "Install for this client at " + path,
            _ => "Copy the bundled doze folder into your client’s skill directory."
        };
        content.Children.Add(new TextBlock { Text = description, TextWrapping = TextWrapping.Wrap, FontSize = 12, Opacity = 0.7 });
        var controls = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (status == "not_installed")
        {
            var install = ActionButton("Install Doze skill", () => bridge.SendAgentAsync("agent-skill-install", name: name));
            install.Click += (_, _) => install.IsEnabled = false;
            controls.Children.Add(install);
        }
        if (status == "update_available")
        {
            var confirmation = new StackPanel { Spacing = 8, Visibility = Visibility.Collapsed };
            confirmation.Children.Add(new TextBlock { Text = "Update replaces the bundled instruction files. Your entire current copy is saved in a separate backup folder; extra files are retained. Local instruction edits are not merged.", TextWrapping = TextWrapping.Wrap });
            var confirm = ActionButton("Confirm update", () => bridge.SendAgentAsync("agent-skill-update", name: name));
            confirm.Click += (_, _) => confirm.IsEnabled = false;
            var choices = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            choices.Children.Add(confirm);
            choices.Children.Add(ActionButton("Cancel", () => { confirmation.Visibility = Visibility.Collapsed; return Task.CompletedTask; }));
            confirmation.Children.Add(choices);
            controls.Children.Add(ActionButton("Review update", () => { confirmation.Visibility = Visibility.Visible; return Task.CompletedTask; }));
            content.Children.Add(confirmation);
        }
        controls.Children.Add(ActionButton("Open skill folder", () => bridge.SendAgentAsync("agent-skill-folder")));
        content.Children.Add(controls);
        var message = snapshot["agentSkillMessage"]?.GetValue<string>();
        if (message?.StartsWith("Doze skill", StringComparison.Ordinal) == true)
            content.Children.Add(new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap, FontSize = 12 });
    }

    private StackPanel AgentPermissionContent(JsonObject client)
    {
        var content = new StackPanel { Spacing = 16, Width = 480, MaxWidth = 480 };
        content.Children.Add(new TextBlock { Text = "Allow automatically when a switch is on. Otherwise, Doze asks for each session. Changes apply immediately and cancel this agent’s active sessions.", TextWrapping = TextWrapping.Wrap });
        var grid = new Grid { ColumnSpacing = 24, RowSpacing = 16 };
        grid.ColumnDefinitions.Add(new ColumnDefinition());
        grid.ColumnDefinitions.Add(new ColumnDefinition());
        var entries = new List<(string Label, string? Action, bool Allowed)>
        { ("Keep awake", null, client["keepAwake"]?.GetValue<bool>() == true) };
        entries.AddRange(Actions.Select(action => (Labels.Action(action), (string?)action,
            client["actions"]!.AsArray().Any(a => a!.GetValue<string>() == action))));
        for (var index = 0; index < entries.Count; index++)
        {
            if (index % 2 == 0) grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var (label, action, allowed) = entries[index];
            var toggle = new ToggleSwitch { Header = label, IsOn = allowed, OnContent = "Allowed", OffContent = "Ask each time" };
            AutomationProperties.SetName(toggle, $"Automatically allow {label}");
            toggle.Toggled += async (_, _) =>
            {
                toggle.IsEnabled = false;
                try { await bridge.SendAgentAsync("agent-permission", id: client["id"]!.GetValue<string>(), action: action); }
                catch (Exception error) { toggle.IsEnabled = true; Notify("Couldn't change permission", error.Message, InfoBarSeverity.Error); }
            };
            Grid.SetRow(toggle, index / 2); Grid.SetColumn(toggle, index % 2); grid.Children.Add(toggle);
        }
        content.Children.Add(grid);
        return content;
    }
}
