using FastFill.Core;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Http.Features;
using Microsoft.Extensions.DependencyInjection;
using SkiaSharp;

namespace FastFill.Checks;

internal static class SnapLab
{
    private const long MaximumUploadBytes = 650L * 1024 * 1024;
    private const string DefaultContentType =
        "application/octet-stream";

    public static async Task RunAsync(string url)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(url);
        var builder = WebApplication.CreateSlimBuilder();
        builder.WebHost.UseUrls(url);
        builder.WebHost.ConfigureKestrel(options =>
        {
            options.Limits.MaxRequestBodySize = MaximumUploadBytes;
        });
        builder.Services.Configure<FormOptions>(options =>
        {
            options.MultipartBodyLengthLimit = MaximumUploadBytes;
        });

        await using var app = builder.Build();
        var session = new SnapLabSession();
        app.Use(async (context, next) =>
        {
            context.Response.Headers.XContentTypeOptions = "nosniff";
            context.Response.Headers.CacheControl = "no-store";
            await next();
        });
        app.MapGet(
            "/",
            () => Results.Text(
                ReadPage(),
                "text/html; charset=utf-8"));
        app.MapPost(
            "/api/load",
            (HttpRequest request, CancellationToken token) =>
                LoadAsync(session, request, token));
        app.MapGet(
            "/api/page/{index:int}",
            (int index) => PageResult(session, index));
        app.MapGet(
            "/api/features/{index:int}",
            (int index) => FeaturesResult(session, index));
        app.MapGet(
            "/api/snap/{index:int}",
            (int index,
                float x,
                float y,
                float radius,
                float fontSize,
                string tool) =>
                SnapResult(
                    session,
                    index,
                    x,
                    y,
                    radius,
                    fontSize,
                    tool));

        Console.WriteLine($"FastFill Snap Lab: {url}");
        Console.WriteLine("Open http://localhost:5077 in a browser.");
        await app.RunAsync();
    }

    private static async Task<IResult> LoadAsync(
        SnapLabSession session,
        HttpRequest request,
        CancellationToken cancellationToken)
    {
        try
        {
            if (!request.HasFormContentType)
            {
                return Error("Choose a JPEG, PNG, or .docscan file.");
            }

            var form = await request.ReadFormAsync(cancellationToken);
            var file = form.Files.GetFile("file");
            if (file is null)
            {
                return Error("Upload field 'file' is missing.");
            }

            var pages = await LoadPagesAsync(file, cancellationToken);
            session.Replace(pages);
            return Results.Json(new
            {
                pages = pages.Select((page, index) => new
                {
                    index,
                    page.Label,
                    page.Width,
                    page.Height,
                    lineCount = page.Features.HorizontalLines.Count,
                    boxCount = page.Features.Boxes.Count,
                }),
            });
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception exception)
        {
            return Error(exception.Message);
        }
    }

    private static IResult PageResult(
        SnapLabSession session,
        int index)
    {
        var page = session.Get(index);
        return page is null
            ? Results.NotFound()
            : Results.Bytes(page.EncodedImage, page.ContentType);
    }

    private static IResult FeaturesResult(
        SnapLabSession session,
        int index)
    {
        var page = session.Get(index);
        if (page is null)
        {
            return Results.NotFound();
        }

        return Results.Json(new
        {
            lines = page.Features.HorizontalLines.Select(LineResult),
            boxes = page.Features.Boxes.Select(RectResult),
        });
    }

    private static IResult SnapResult(
        SnapLabSession session,
        int index,
        float x,
        float y,
        float radius,
        float fontSize,
        string tool)
    {
        var page = session.Get(index);
        if (page is null)
        {
            return Results.NotFound();
        }

        if (!float.IsFinite(x)
            || !float.IsFinite(y)
            || x is < 0 or > 1
            || y is < 0 or > 1
            || !float.IsFinite(radius)
            || radius is < 0.005f or > 0.2f
            || (tool == "text"
                && (!float.IsFinite(fontSize)
                    || fontSize is < 6 or > 144)))
        {
            return Error("Snap parameters are outside their limits.");
        }

        return tool switch
        {
            "text" => TextSnapResult(page, x, y, radius, fontSize),
            "check" or "cross" => ShapeSnapResult(
                page,
                x,
                y,
                radius,
                tool),
            _ => Error("Tool must be text, check, or cross."),
        };
    }

    private static IResult TextSnapResult(
        SnapLabPage page,
        float x,
        float y,
        float radius,
        float fontSize)
    {

        var line = page.Features.FindHorizontalLine(
            new NormalizedPoint(x, y),
            radius);
        if (line is null)
        {
            return Results.Json(new
            {
                tool = "text",
                line = (object?)null,
            });
        }

        var lineY = (line.Start.Y + line.End.Y) / 2;
        var width = Math.Clamp(
            line.End.X - line.Start.X,
            0.08f,
            0.8f);
        var pageHeightPoints = page.Height / 300f * 72;
        var height = fontSize * 1.25f
            / Math.Max(pageHeightPoints, 1);
        return Results.Json(new
        {
            tool = "text",
            line = LineResult(line),
            placement = RectResult(new(
                line.Start.X,
                Math.Max(0, lineY - height),
                width,
                height)),
        });
    }

    private static IResult ShapeSnapResult(
        SnapLabPage page,
        float x,
        float y,
        float radius,
        string tool)
    {
        var candidate = page.Features.FindBox(
            new NormalizedPoint(x, y),
            radius);
        if (candidate is not NormalizedRect box)
        {
            return Results.Json(new
            {
                tool,
                candidate = (object?)null,
            });
        }

        var centerX = box.X + (box.Width / 2);
        var centerY = box.Y + (box.Height / 2);
        var size = Math.Min(
            box.Width * page.Width,
            box.Height * page.Height) * 0.78f;
        var width = Math.Clamp(size / page.Width, 0.005f, 1);
        var height = Math.Clamp(size / page.Height, 0.005f, 1);
        var left = Math.Clamp(centerX - (width / 2), 0, 1 - width);
        var top = Math.Clamp(centerY - (height / 2), 0, 1 - height);
        return Results.Json(new
        {
            tool,
            candidate = RectResult(box),
            placement = RectResult(new(
                left,
                top,
                width,
                height)),
        });
    }

    private static object LineResult(SnapLineFeature line) => new
    {
        start = new { x = line.Start.X, y = line.Start.Y },
        end = new { x = line.End.X, y = line.End.Y },
    };

    private static object RectResult(NormalizedRect rectangle) => new
    {
        x = rectangle.X,
        y = rectangle.Y,
        width = rectangle.Width,
        height = rectangle.Height,
    };

    private static IResult Error(string message) =>
        Results.BadRequest(new { error = message });

    private static async Task<IReadOnlyList<SnapLabPage>> LoadPagesAsync(
        IFormFile file,
        CancellationToken cancellationToken)
    {
        if (file.Length is <= 0 or > MaximumUploadBytes)
        {
            throw new InvalidDataException(
                "Input must be between 1 byte and 650 MiB.");
        }

        var name = Path.GetFileName(file.FileName);
        var extension = Path.GetExtension(name).ToLowerInvariant();
        return extension switch
        {
            ".docscan" => await LoadProjectAsync(
                file,
                cancellationToken),
            ".jpg" or ".jpeg" or ".png" =>
                [await LoadImageAsync(file, name, cancellationToken)],
            ".pdf" => throw new InvalidDataException(
                "Import the PDF in FastFill and save a .docscan first."),
            _ => throw new InvalidDataException(
                "Only JPEG, PNG, and .docscan files are supported."),
        };
    }

    private static async Task<SnapLabPage> LoadImageAsync(
        IFormFile file,
        string label,
        CancellationToken cancellationToken)
    {
        if (file.Length > ProjectValidator.MaximumImageBytes)
        {
            throw new InvalidDataException("Image exceeds 100 MiB.");
        }

        await using var input = file.OpenReadStream();
        using var output = new MemoryStream((int)file.Length);
        await input.CopyToAsync(output, cancellationToken);
        return CreateImagePage(label, output.ToArray());
    }

    private static SnapLabPage CreateImagePage(
        string label,
        byte[] bytes)
    {
        using var data = SKData.CreateCopy(bytes);
        using var codec = SKCodec.Create(data)
            ?? throw new InvalidDataException("Image could not be decoded.");
        var contentType = codec.EncodedFormat switch
        {
            SKEncodedImageFormat.Jpeg => "image/jpeg",
            SKEncodedImageFormat.Png => "image/png",
            _ => DefaultContentType,
        };
        if (contentType == DefaultContentType)
        {
            throw new InvalidDataException("Image must be JPEG or PNG.");
        }

        ProjectValidator.ValidateImageInput(
            bytes.LongLength,
            codec.Info.Width,
            codec.Info.Height);
        return new(
            label,
            bytes,
            contentType,
            codec.Info.Width,
            codec.Info.Height,
            ImageSnapFeatures.Analyze(bytes));
    }

    private static async Task<IReadOnlyList<SnapLabPage>>
        LoadProjectAsync(
            IFormFile file,
            CancellationToken cancellationToken)
    {
        var root = Path.Combine(
            Path.GetTempPath(),
            $"fastfill-snaplab-{Guid.NewGuid():N}");
        var archive = Path.Combine(root, "input.docscan");
        var workspace = Path.Combine(root, "workspace");
        Directory.CreateDirectory(root);
        try
        {
            await using (var output = new FileStream(
                archive,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                64 * 1024,
                FileOptions.Asynchronous))
            {
                await file.CopyToAsync(output, cancellationToken);
            }

            var loaded = await ProjectArchive.LoadAsync(
                archive,
                workspace,
                cancellationToken);
            var pages = new List<SnapLabPage>();
            for (var index = 0;
                index < loaded.Project.Pages.Count;
                index++)
            {
                var processed = await WorkspaceStore.ProcessPageAsync(
                    workspace,
                    loaded.Project.Pages[index],
                    cancellationToken);
                pages.Add(new(
                    $"Page {index + 1}",
                    processed.EncodedPng,
                    "image/png",
                    processed.Width,
                    processed.Height,
                    ImageSnapFeatures.Analyze(processed.EncodedPng)));
            }

            if (pages.Count == 0)
            {
                throw new InvalidDataException("Project has no pages.");
            }

            return pages;
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    private static string ReadPage()
    {
        using var stream = typeof(SnapLab).Assembly
            .GetManifestResourceStream("FastFill.Checks.SnapLab.html")
            ?? throw new InvalidOperationException(
                "Snap Lab page is missing.");
        using var reader = new StreamReader(stream);
        return reader.ReadToEnd();
    }

    private sealed class SnapLabSession
    {
        private readonly object _gate = new();
        private IReadOnlyList<SnapLabPage> _pages = [];

        public void Replace(IReadOnlyList<SnapLabPage> pages)
        {
            lock (_gate)
            {
                _pages = pages;
            }
        }

        public SnapLabPage? Get(int index)
        {
            lock (_gate)
            {
                return index >= 0 && index < _pages.Count
                    ? _pages[index]
                    : null;
            }
        }
    }

    private sealed record SnapLabPage(
        string Label,
        byte[] EncodedImage,
        string ContentType,
        int Width,
        int Height,
        ImageSnapFeatures Features);
}
