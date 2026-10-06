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

    /// Engine messages: replies to commands, and `state` whenever the engine changes.
    public async Task ListenAsync(Action<JsonObject> receive)
    {
        while (await input.ReadLineAsync() is { } line)
            receive(Parse(line));
        // The app coordinator cleans up native listeners when the Rust pipe closes.
    }

    public Task SendAsync(string command) => SendCommandAsync(command);

    /// Any engine command with optional fields such as seconds, action, key or id. The engine
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
