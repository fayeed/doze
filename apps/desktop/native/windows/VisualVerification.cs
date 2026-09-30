using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace Doze.SettingsUi;

internal static class VisualVerification
{
    public static async Task SaveAsync(FrameworkElement root, string path)
    {
        root.UpdateLayout();
        var bitmap = new RenderTargetBitmap();
        await bitmap.RenderAsync(root);
        var pixels = await bitmap.GetPixelsAsync();
        var bytes = new byte[pixels.Length];
        using (var reader = DataReader.FromBuffer(pixels)) reader.ReadBytes(bytes);
        using var stream = new InMemoryRandomAccessStream();
        var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, stream);
        encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied,
            (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, 96, 96, bytes);
        await encoder.FlushAsync();
        using var fileReader = new DataReader(stream.GetInputStreamAt(0));
        await fileReader.LoadAsync((uint)stream.Size);
        var png = new byte[(int)stream.Size];
        fileReader.ReadBytes(png);
        await File.WriteAllBytesAsync(path, png);
    }
}
