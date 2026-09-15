using System.Runtime.InteropServices;
using FastFill.Core;
using Microsoft.UI.Xaml.Controls;
using Windows.Devices.Enumeration;
using Windows.Graphics.Imaging;
using Windows.Media.Capture;
using Windows.Media.Capture.Frames;
using Windows.Media.Core;
using Windows.Media.MediaProperties;
using Windows.Storage.Streams;

namespace FastFill.App;

internal sealed record CameraChoice(
    string Id,
    string Name,
    CameraPosition Position);

internal sealed class CameraFrameSource : IFrameSource
{
    private readonly CameraChoice _choice;
    private readonly MediaPlayerElement _preview;
    private MediaCapture? _capture;
    private MediaFrameReader? _reader;
    private FramePacket? _latestFrame;
    private long _lastFrameTicks;
    private int _copyingFrame;

    public CameraFrameSource(
        CameraChoice choice,
        MediaPlayerElement preview)
    {
        _choice = choice;
        _preview = preview;
    }

    public event Action<FramePacket>? FrameArrived;
    public event Action<string>? Failed;

    public CameraPosition Position => _choice.Position;

    public static async Task<IReadOnlyList<CameraChoice>> FindAsync()
    {
        var devices = await DeviceInformation.FindAllAsync(
            DeviceClass.VideoCapture);
        return devices.Select(device => new CameraChoice(
                device.Id,
                device.Name,
                ToPosition(device.EnclosureLocation?.Panel)))
            .OrderByDescending(choice =>
                choice.Position == CameraPosition.Back)
            .ThenBy(choice => choice.Name, StringComparer.CurrentCulture)
            .ToArray();
    }

    public async Task StartAsync(
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (_capture is not null)
        {
            return;
        }

        var capture = new MediaCapture();
        capture.Failed += Capture_Failed;
        try
        {
            await capture.InitializeAsync(
                new MediaCaptureInitializationSettings
                {
                    VideoDeviceId = _choice.Id,
                    StreamingCaptureMode = StreamingCaptureMode.Video,
                    MemoryPreference = MediaCaptureMemoryPreference.Cpu,
                    SharingMode = MediaCaptureSharingMode.ExclusiveControl,
                });
            cancellationToken.ThrowIfCancellationRequested();
            var colorSources = capture.FrameSources.Values
                .Where(candidate =>
                    candidate.Info.SourceKind == MediaFrameSourceKind.Color)
                .ToArray();
            var source = colorSources.FirstOrDefault(candidate =>
                    candidate.Info.MediaStreamType
                        == MediaStreamType.VideoPreview)
                ?? colorSources.FirstOrDefault(candidate =>
                    candidate.Info.MediaStreamType
                        == MediaStreamType.VideoRecord)
                ?? colorSources.FirstOrDefault()
                ?? throw new InvalidOperationException(
                    "Camera has no color preview stream.");
            _preview.AutoPlay = true;
            _preview.Source = MediaSource.CreateFromMediaFrameSource(source);
            var reader = await capture.CreateFrameReaderAsync(
                source,
                MediaEncodingSubtypes.Bgra8);
            reader.AcquisitionMode =
                MediaFrameReaderAcquisitionMode.Realtime;
            reader.FrameArrived += Reader_FrameArrived;
            var status = await reader.StartAsync();
            if (status != MediaFrameReaderStartStatus.Success)
            {
                reader.FrameArrived -= Reader_FrameArrived;
                reader.Dispose();
                throw new InvalidOperationException(
                    $"Camera frame reader failed: {status}.");
            }

            _capture = capture;
            _reader = reader;
        }
        catch
        {
            capture.Failed -= Capture_Failed;
            capture.Dispose();
            throw;
        }
    }

    public async Task StopAsync(
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var reader = _reader;
        _reader = null;
        if (reader is not null)
        {
            reader.FrameArrived -= Reader_FrameArrived;
            await reader.StopAsync();
            reader.Dispose();
        }

        _preview.Source = null;
        var capture = _capture;
        _capture = null;
        if (capture is not null)
        {
            capture.Failed -= Capture_Failed;
            capture.Dispose();
        }
    }

    public Task<FramePacket> CaptureAsync(
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return Task.FromResult(_latestFrame
            ?? throw new InvalidOperationException(
                "Camera has not delivered a preview frame."));
    }

    public async Task<byte[]> CaptureJpegAsync(
        CancellationToken cancellationToken = default)
    {
        var capture = _capture
            ?? throw new InvalidOperationException("Camera is not running.");
        using var stream = new InMemoryRandomAccessStream();
        await capture.CapturePhotoToStreamAsync(
            ImageEncodingProperties.CreateJpeg(),
            stream);
        cancellationToken.ThrowIfCancellationRequested();
        if (stream.Size > ProjectValidator.MaximumImageBytes)
        {
            throw new InvalidDataException("Camera image exceeds 100 MiB.");
        }

        stream.Seek(0);
        using var reader = new DataReader(stream.GetInputStreamAt(0));
        await reader.LoadAsync((uint)stream.Size);
        var result = new byte[(int)stream.Size];
        reader.ReadBytes(result);
        return result;
    }

    public async ValueTask DisposeAsync()
    {
        await StopAsync();
        GC.SuppressFinalize(this);
    }

    private void Reader_FrameArrived(
        MediaFrameReader sender,
        MediaFrameArrivedEventArgs args)
    {
        var now = DateTime.UtcNow.Ticks;
        if (now - Interlocked.Read(ref _lastFrameTicks)
                < TimeSpan.FromMilliseconds(100).Ticks
            || Interlocked.Exchange(ref _copyingFrame, 1) != 0)
        {
            return;
        }

        Interlocked.Exchange(ref _lastFrameTicks, now);
        try
        {
            using var reference = sender.TryAcquireLatestFrame();
            using var source = reference?.VideoMediaFrame?.SoftwareBitmap;
            if (source is null)
            {
                return;
            }

            using var bitmap = SoftwareBitmap.Convert(
                source,
                BitmapPixelFormat.Bgra8,
                BitmapAlphaMode.Premultiplied);
            var packet = CopyFrame(bitmap);
            _latestFrame = packet;
            FrameArrived?.Invoke(packet);
        }
        finally
        {
            Interlocked.Exchange(ref _copyingFrame, 0);
        }
    }

    private unsafe FramePacket CopyFrame(SoftwareBitmap bitmap)
    {
        using var buffer = bitmap.LockBuffer(BitmapBufferAccessMode.Read);
        using var reference = buffer.CreateReference();
        var access = (IMemoryBufferByteAccess)reference;
        access.GetBuffer(out var data, out _);
        var description = buffer.GetPlaneDescription(0);
        var stride = checked(bitmap.PixelWidth * 4);
        var pixels = new byte[checked(stride * bitmap.PixelHeight)];
        for (var row = 0; row < bitmap.PixelHeight; row++)
        {
            var source = new ReadOnlySpan<byte>(
                data + description.StartIndex
                    + row * description.Stride,
                stride);
            source.CopyTo(pixels.AsSpan(row * stride, stride));
        }

        return new FramePacket(
            DateTimeOffset.UtcNow,
            pixels,
            bitmap.PixelWidth,
            bitmap.PixelHeight,
            stride,
            0,
            Position == CameraPosition.Front,
            Position);
    }

    private void Capture_Failed(
        MediaCapture sender,
        MediaCaptureFailedEventArgs errorEventArgs)
    {
        _preview.DispatcherQueue.TryEnqueue(() =>
        {
            _preview.Source = null;
            Failed?.Invoke(errorEventArgs.Message);
        });
    }

    private static CameraPosition ToPosition(
        Windows.Devices.Enumeration.Panel? panel) => panel switch
    {
        Windows.Devices.Enumeration.Panel.Front => CameraPosition.Front,
        Windows.Devices.Enumeration.Panel.Back => CameraPosition.Back,
        _ => CameraPosition.Unknown,
    };

    [ComImport]
    [Guid("5b0d3235-4dba-4d44-865e-8f1d0e4fd04d")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private unsafe interface IMemoryBufferByteAccess
    {
        void GetBuffer(out byte* buffer, out uint capacity);
    }
}
