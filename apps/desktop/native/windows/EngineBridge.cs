using System.Text.Json;
using System.Text.Json.Nodes;

namespace Doze.SettingsUi;

// Private inherited pipes connect this window to the Rust owner. No ports, files, or IPC service.
public sealed class EngineBridge
{
    private readonly StreamReader input = new(Console.OpenStandardInput());
    private readonly StreamWriter output = new(Console.OpenStandardOutput()) { AutoFlush = true };
    private readonly SemaphoreSlim writeLock = new(1);
    public static JsonSerializerOptions Json { get; } = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

    public async Task<JsonObject> ReadInitialAsync() => Parse(await input.ReadLineAsync());

    public async Task ListenAsync(Action<JsonObject> receive)
    {
        while (await input.ReadLineAsync() is { } line)
            receive(Parse(line));
        // The app coordinator cleans up native listeners when the Rust pipe closes.
    }

    public async Task SendAsync(string command, Preferences? settings = null)
    {
        await writeLock.WaitAsync();
        try
        {
            await output.WriteLineAsync(JsonSerializer.Serialize(new { command, settings }, Json));
        }
        finally { writeLock.Release(); }
    }

    public async Task SendAgentAsync(string command, string? id = null, string? decision = null, string? name = null, string? action = null, int? seconds = null)
    {
        await writeLock.WaitAsync();
        try { await output.WriteLineAsync(JsonSerializer.Serialize(new { command, id, decision, name, action, agent_seconds = seconds }, Json)); }
        finally { writeLock.Release(); }
    }

    /// Any engine command with optional fields such as seconds, action or id. The engine
    /// validates every field and rejects unknown ones.
    public async Task SendCommandAsync(string command, JsonObject? fields = null)
    {
        var message = new JsonObject { ["command"] = command };
        if (fields is not null)
            foreach (var (key, value) in fields) message[key] = value?.DeepClone();
        await writeLock.WaitAsync();
        try { await output.WriteLineAsync(message.ToJsonString()); }
        finally { writeLock.Release(); }
    }

    private static JsonObject Parse(string? line) =>
        JsonNode.Parse(line ?? throw new IOException("Doze engine disconnected."))?.AsObject()
        ?? throw new IOException("Invalid engine response.");
}

public sealed record Preferences
{
    public string Theme { get; set; } = "system";
    public bool LaunchAtStartup { get; set; }
    public bool StartMinimized { get; set; } = true;
    public bool Notifications { get; set; } = true;
    public string DefaultAction { get; set; } = "sleep";
    public int SilenceSeconds { get; set; } = 60;
    public int IdleSeconds { get; set; } = 300;
    public int CountdownSeconds { get; set; } = 300;
    public string PlaybackAction { get; set; } = "sleep";
    public bool Logging { get; set; }
    public bool AllowDisplaySleep { get; set; }
    public int DefaultAwakeMinutes { get; set; } = 30;
    public int DefaultTimerMinutes { get; set; } = 30;
    // Used by the macOS menu bar; kept so Windows saves preserve it.
    public bool MenuBarTime { get; set; } = true;
}
