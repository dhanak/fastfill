using System.IO.Compression;
using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using SkiaSharp;

namespace FastFill.Core;

public static class ProjectJson
{
    public static JsonSerializerOptions Options { get; } = CreateOptions();

    public static FastFillProject Clone(FastFillProject project)
    {
        var bytes = JsonSerializer.SerializeToUtf8Bytes(project, Options);
        return JsonSerializer.Deserialize<FastFillProject>(bytes, Options)
            ?? throw new InvalidOperationException("Project clone failed.");
    }

    private static JsonSerializerOptions CreateOptions() => new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true,
        AllowTrailingCommas = false,
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
        NumberHandling = JsonNumberHandling.Strict,
    };
}

public static class ProjectValidator
{
    public const long MaximumImageBytes = 100L * 1024 * 1024;
    public const long MaximumImagePixels = 50_000_000;
    public const int MaximumAnnotationsPerPage = 10_000;
    public const int MaximumInkPoints = 100_000;
    public const int MaximumTextLength = 10_000;

    public static void Validate(FastFillProject project)
    {
        ArgumentNullException.ThrowIfNull(project);
        if (project.SchemaVersion != FastFillProject.CurrentSchemaVersion)
        {
            throw new InvalidDataException(
                project.SchemaVersion > FastFillProject.CurrentSchemaVersion
                    ? "Project needs a newer FastFill version."
                    : "Project schema is unsupported.");
        }

        if (project.Pages is null)
        {
            throw new InvalidDataException("Project pages are missing.");
        }

        if (project.Pages.Count > FastFillProject.MaximumPages)
        {
            throw new InvalidDataException(
                $"A project may contain at most "
                + $"{FastFillProject.MaximumPages} pages.");
        }

        if (project.Title is null || project.Title.Length > 200)
        {
            throw new InvalidDataException("Project title is too long.");
        }

        var pageIds = new HashSet<Guid>();
        foreach (var page in project.Pages)
        {
            ValidatePage(page, pageIds);
        }
    }

    public static void ValidateImageInput(
        long byteLength,
        int width,
        int height)
    {
        if (byteLength is <= 0 or > MaximumImageBytes)
        {
            throw new InvalidDataException(
                "Image must be between 1 byte and 100 MiB.");
        }

        if (width <= 0
            || height <= 0
            || (long)width * height > MaximumImagePixels)
        {
            throw new InvalidDataException(
                "Image dimensions exceed the 50-megapixel limit.");
        }
    }

    public static void ValidateEncodedImage(byte[] encodedImage)
    {
        ArgumentNullException.ThrowIfNull(encodedImage);
        using var stream = new MemoryStream(
            encodedImage,
            writable: false);
        _ = InspectEncodedImage(stream, encodedImage.LongLength);
    }

    public static async Task<byte[]> ReadEncodedImageAsync(
        Stream stream,
        long expectedLength,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(stream);
        if (!stream.CanRead)
        {
            throw new ArgumentException(
                "Image stream must be readable.",
                nameof(stream));
        }

        if (expectedLength is <= 0 or > MaximumImageBytes)
        {
            throw new InvalidDataException(
                "Image must be between 1 byte and 100 MiB.");
        }

        if (stream.CanSeek
            && stream.Length - stream.Position != expectedLength)
        {
            throw new InvalidDataException(
                "Image size changed before it could be read.");
        }

        var bytes = new byte[checked((int)expectedLength)];
        try
        {
            await stream.ReadExactlyAsync(bytes, cancellationToken);
        }
        catch (EndOfStreamException exception)
        {
            throw new InvalidDataException(
                "Image ended before it could be read.",
                exception);
        }

        var trailing = new byte[1];
        if (await stream.ReadAsync(trailing, cancellationToken) != 0)
        {
            throw new InvalidDataException(
                "Image grew while it was being read.");
        }

        ValidateEncodedImage(bytes);
        return bytes;
    }

    internal static void ValidateImageAsset(
        string path,
        DocumentPage page)
    {
        var info = new FileInfo(path);
        using var stream = info.OpenRead();
        var image = InspectEncodedImage(stream, info.Length);
        if (image.Width != page.SourcePixelWidth
            || image.Height != page.SourcePixelHeight)
        {
            throw new InvalidDataException(
                $"Page asset dimensions do not match the manifest: "
                + page.SourceAsset);
        }

        var extension = Path.GetExtension(page.SourceAsset);
        var formatMatches = image.Format switch
        {
            SKEncodedImageFormat.Jpeg => extension.Equals(
                ".jpg",
                StringComparison.OrdinalIgnoreCase)
                || extension.Equals(
                    ".jpeg",
                    StringComparison.OrdinalIgnoreCase),
            SKEncodedImageFormat.Png => extension.Equals(
                ".png",
                StringComparison.OrdinalIgnoreCase),
            _ => false,
        };
        if (!formatMatches)
        {
            throw new InvalidDataException(
                $"Page asset format does not match its extension: "
                + page.SourceAsset);
        }
    }

    private static EncodedImageInfo InspectEncodedImage(
        Stream stream,
        long byteLength)
    {
        if (byteLength is <= 0 or > MaximumImageBytes)
        {
            throw new InvalidDataException(
                "Image must be between 1 byte and 100 MiB.");
        }

        // SKCodec reads metadata without allocating the decoded pixel buffer.
        using var codec = CreateCodec(stream);
        var info = codec.Info;
        ValidateImageInput(byteLength, info.Width, info.Height);
        if (codec.EncodedFormat is not (
            SKEncodedImageFormat.Jpeg or SKEncodedImageFormat.Png))
        {
            throw new InvalidDataException(
                "Image must be JPEG or PNG.");
        }

        return new(info.Width, info.Height, codec.EncodedFormat);
    }

    private static SKCodec CreateCodec(Stream stream)
    {
        try
        {
            return SKCodec.Create(stream)
                ?? throw new InvalidDataException(
                    "Image header is invalid or unsupported.");
        }
        catch (InvalidDataException)
        {
            throw;
        }
        catch (Exception exception)
            when (exception is not OutOfMemoryException)
        {
            throw new InvalidDataException(
                "Image header is invalid or unsupported.",
                exception);
        }
    }

    private readonly record struct EncodedImageInfo(
        int Width,
        int Height,
        SKEncodedImageFormat Format);

    private static void ValidatePage(
        DocumentPage page,
        HashSet<Guid> pageIds)
    {
        if (page is null)
        {
            throw new InvalidDataException("Project contains a null page.");
        }

        if (!pageIds.Add(page.Id))
        {
            throw new InvalidDataException("Page IDs must be unique.");
        }

        ValidateAssetName(page.SourceAsset);
        ValidateImageInput(
            Math.Max(1, page.SourcePixelWidth * (long)page.SourcePixelHeight),
            page.SourcePixelWidth,
            page.SourcePixelHeight);
        if (page.Crop is null
            || page.Crop.Points.Any(point => !IsCropPoint(point))
            || !page.Crop.IsConvex())
        {
            throw new InvalidDataException(
                "Crop corners must be finite, inside the page, and convex.");
        }

        if (page.RotationQuarterTurns is < 0 or > 3)
        {
            throw new InvalidDataException("Page rotation is invalid.");
        }

        if (page.Filter is null
            || !Enum.IsDefined(page.Filter.Mode)
            || page.Filter.Contrast is < -100 or > 100)
        {
            throw new InvalidDataException("Page filter is invalid.");
        }

        if (page.Annotations is null
            || page.Annotations.Count > MaximumAnnotationsPerPage)
        {
            throw new InvalidDataException(
                "Page annotations are missing or too numerous.");
        }

        var renderSizes = GetRenderSizes(page);
        var annotationIds = new HashSet<Guid>();
        foreach (var annotation in page.Annotations)
        {
            if (annotation is null)
            {
                throw new InvalidDataException(
                    "Page contains a null annotation.");
            }

            if (!annotationIds.Add(annotation.Id))
            {
                throw new InvalidDataException(
                    "Annotation IDs must be unique per page.");
            }

            ValidateAnnotation(annotation);
            ValidateRendererGeometry(
                annotation,
                renderSizes.Page.Width,
                renderSizes.Page.Height);
            ValidateRendererGeometry(
                annotation,
                renderSizes.Preview.Width,
                renderSizes.Preview.Height);
        }
    }

    public static void ValidateAnnotation(
        Annotation annotation,
        DocumentPage page)
    {
        ArgumentNullException.ThrowIfNull(page);
        var renderSizes = GetRenderSizes(page);
        ValidateAnnotation(annotation);
        ValidateRendererGeometry(
            annotation,
            renderSizes.Page.Width,
            renderSizes.Page.Height);
        ValidateRendererGeometry(
            annotation,
            renderSizes.Preview.Width,
            renderSizes.Preview.Height);
    }

    public static void ValidateAnnotation(Annotation annotation)
    {
        ArgumentNullException.ThrowIfNull(annotation);
        if (!float.IsFinite(annotation.StrokeWidth)
            || annotation.StrokeWidth is < 0.25f or > 40)
        {
            throw new InvalidDataException(
                "Annotation stroke width is invalid.");
        }

        ValidateTransform(annotation.Transform);
        switch (annotation)
        {
            case FreehandAnnotation ink:
                if (ink.Points is null
                    || ink.Points.Count is 0 or > MaximumInkPoints
                    || ink.Points.Any(point => !IsFinite(point)))
                {
                    throw new InvalidDataException(
                        "Freehand annotation points are invalid.");
                }

                break;
            case TextAnnotation text:
                ValidateTextAnnotation(text);
                break;
            case ShapeAnnotation shape:
                if (!Enum.IsDefined(shape.Shape)
                    || !IsFinite(shape.Start)
                    || !IsFinite(shape.End))
                {
                    throw new InvalidDataException(
                        "Shape annotation geometry is invalid.");
                }

                break;
            default:
                throw new InvalidDataException(
                    "Annotation type is invalid.");
        }

        var bounds = AnnotationGeometry.TransformedBounds(annotation);
        if (!IsFinite(bounds)
            || bounds.Right < 0
            || bounds.Bottom < 0
            || bounds.X > 1
            || bounds.Y > 1)
        {
            throw new InvalidDataException(
                "Annotation must be at least partly on the page.");
        }
    }

    private static void ValidateRendererGeometry(
        Annotation annotation,
        float pageWidth,
        float pageHeight)
    {
        var bounds = AnnotationGeometry.Bounds(annotation);
        var transformedBounds = AnnotationGeometry.TransformedBounds(
            annotation);
        if (!IsRenderable(bounds, pageWidth, pageHeight)
            || !IsRenderable(
                transformedBounds,
                pageWidth,
                pageHeight)
            || !IsRenderable(
                annotation.Transform,
                pageWidth,
                pageHeight)
            || AnnotationGeometry.BoundsCorners(bounds).Any(point =>
                !IsRenderable(
                    point,
                    annotation.Transform,
                    pageWidth,
                    pageHeight)))
        {
            throw new InvalidDataException(
                "Annotation exceeds the renderer's numeric range.");
        }

        switch (annotation)
        {
            case FreehandAnnotation ink
                when ink.Points.Any(point =>
                    !IsRenderable(
                        point,
                        annotation.Transform,
                        pageWidth,
                        pageHeight)):
                throw new InvalidDataException(
                    "Freehand annotation exceeds the renderer's "
                    + "numeric range.");
            case ShapeAnnotation shape
                when !IsRenderable(
                    shape.Start,
                    annotation.Transform,
                    pageWidth,
                    pageHeight)
                || !IsRenderable(
                    shape.End,
                    annotation.Transform,
                    pageWidth,
                    pageHeight)
                || (shape.Shape == ShapeKind.Arrow
                    && !IsRenderableArrow(
                        shape,
                        pageWidth,
                        pageHeight)):
                throw new InvalidDataException(
                    "Shape annotation exceeds the renderer's numeric "
                    + "range.");
        }
    }

    private static void ValidateTextAnnotation(TextAnnotation text)
    {
        ArgumentNullException.ThrowIfNull(text);
        if (text.Text is null || text.Text.Length > MaximumTextLength)
        {
            throw new InvalidDataException(
                $"Text annotation exceeds {MaximumTextLength} characters.");
        }

        if (!float.IsFinite(text.FontSize)
            || text.FontSize is < 6 or > 144)
        {
            throw new InvalidDataException(
                "Text annotation font size is invalid.");
        }

        if (string.IsNullOrWhiteSpace(text.FontFamily)
            || text.FontFamily.Length > 100)
        {
            throw new InvalidDataException(
                "Text annotation font family is invalid.");
        }

        if (!float.IsFinite(text.MaximumWidth)
            || text.MaximumWidth is <= 0 or > 1)
        {
            throw new InvalidDataException(
                "Text annotation maximum width is invalid.");
        }

        if (!IsTextBounds(text.Bounds))
        {
            throw new InvalidDataException(
                "Text annotation bounds are invalid.");
        }
    }

    private static (
        (float Width, float Height) Page,
        (float Width, float Height) Preview)
        GetRenderSizes(DocumentPage page)
    {
        if (page.Crop is null)
        {
            throw new InvalidDataException("Page crop is missing.");
        }

        var dimensions = ImageProcessor.GetProcessedDimensions(
            page.SourcePixelWidth,
            page.SourcePixelHeight,
            page.Crop,
            page.RotationQuarterTurns);
        (float Width, float Height) pageSize = (
            dimensions.Width / PdfExporter.DotsPerInch * 72,
            dimensions.Height / PdfExporter.DotsPerInch * 72);
        var previewPixels = PageRenderer.GetPreviewDimensions(
            dimensions.Width,
            dimensions.Height);
        (float Width, float Height) preview = (
            PageRenderer.PreviewPageWidthPoints,
            PageRenderer.PreviewPageWidthPoints
                * previewPixels.Height
                / previewPixels.Width);
        return (Page: pageSize, Preview: preview);
    }

    private static bool IsRenderable(
        NormalizedPoint point,
        float pageWidth,
        float pageHeight) =>
        float.IsFinite(point.X * pageWidth)
        && float.IsFinite(point.Y * pageHeight);

    private static bool IsRenderable(
        NormalizedPoint point,
        AffineTransform transform,
        float pageWidth,
        float pageHeight)
    {
        var x = point.X * pageWidth;
        var y = point.Y * pageHeight;
        var skewX = transform.M21 * pageWidth / pageHeight;
        var skewY = transform.M12 * pageHeight / pageWidth;
        var scaledX = x * transform.M11;
        var skewedX = y * skewX;
        var scaledY = y * transform.M22;
        var skewedY = x * skewY;
        var translatedX = scaledX + skewedX;
        var translatedY = skewedY + scaledY;
        return IsRenderable(point, pageWidth, pageHeight)
            && float.IsFinite(scaledX)
            && float.IsFinite(skewedX)
            && float.IsFinite(scaledY)
            && float.IsFinite(skewedY)
            && float.IsFinite(translatedX)
            && float.IsFinite(translatedY)
            && float.IsFinite(
                translatedX + transform.M31 * pageWidth)
            && float.IsFinite(
                translatedY + transform.M32 * pageHeight);
    }

    private static bool IsRenderableArrow(
        ShapeAnnotation arrow,
        float pageWidth,
        float pageHeight)
    {
        var startX = arrow.Start.X * pageWidth;
        var startY = arrow.Start.Y * pageHeight;
        var deltaX = arrow.End.X * pageWidth - startX;
        var deltaY = arrow.End.Y * pageHeight - startY;
        return float.IsFinite(deltaX)
            && float.IsFinite(deltaY)
            && float.IsFinite(deltaX * deltaX)
            && float.IsFinite(deltaY * deltaY)
            && float.IsFinite(
                deltaX * deltaX + deltaY * deltaY);
    }

    private static bool IsRenderable(
        NormalizedRect bounds,
        float pageWidth,
        float pageHeight)
    {
        var left = bounds.X * pageWidth;
        var top = bounds.Y * pageHeight;
        var right = bounds.Right * pageWidth;
        var bottom = bounds.Bottom * pageHeight;
        var horizontalCenter = (bounds.X + bounds.Right)
            / 2
            * pageWidth;
        return float.IsFinite(left)
            && float.IsFinite(top)
            && float.IsFinite(right)
            && float.IsFinite(bottom)
            && float.IsFinite(right - left)
            && float.IsFinite(bottom - top)
            && float.IsFinite(horizontalCenter);
    }

    private static bool IsRenderable(
        AffineTransform transform,
        float pageWidth,
        float pageHeight)
    {
        var skewX = transform.M21 * pageWidth / pageHeight;
        var skewY = transform.M12 * pageHeight / pageWidth;
        return float.IsFinite(skewX)
            && float.IsFinite(skewY)
            && float.IsFinite(transform.M31 * pageWidth)
            && float.IsFinite(transform.M32 * pageHeight);
    }

    private static bool IsCropPoint(NormalizedPoint point) =>
        float.IsFinite(point.X)
        && float.IsFinite(point.Y)
        && point.X is >= 0 and <= 1
        && point.Y is >= 0 and <= 1;

    private static bool IsFinite(NormalizedPoint point) =>
        float.IsFinite(point.X) && float.IsFinite(point.Y);

    private static bool IsTextBounds(NormalizedRect bounds) =>
        IsFinite(bounds)
        && bounds.Width > 0
        && bounds.Height > 0;

    private static bool IsFinite(NormalizedRect bounds) =>
        float.IsFinite(bounds.X)
        && float.IsFinite(bounds.Y)
        && float.IsFinite(bounds.Width)
        && float.IsFinite(bounds.Height)
        && float.IsFinite(bounds.Right)
        && float.IsFinite(bounds.Bottom);

    private static void ValidateTransform(AffineTransform transform)
    {
        var matrix = transform.Matrix;
        var determinant = matrix.GetDeterminant();
        if (!float.IsFinite(transform.M11)
            || !float.IsFinite(transform.M12)
            || !float.IsFinite(transform.M21)
            || !float.IsFinite(transform.M22)
            || !float.IsFinite(transform.M31)
            || !float.IsFinite(transform.M32)
            || !float.IsFinite(determinant)
            || determinant == 0
            || !Matrix3x2.Invert(matrix, out var inverse)
            || !IsFinite(inverse))
        {
            throw new InvalidDataException(
                "Annotation transform is invalid.");
        }
    }

    private static bool IsFinite(Matrix3x2 matrix) =>
        float.IsFinite(matrix.M11)
        && float.IsFinite(matrix.M12)
        && float.IsFinite(matrix.M21)
        && float.IsFinite(matrix.M22)
        && float.IsFinite(matrix.M31)
        && float.IsFinite(matrix.M32);

    private static void ValidateAssetName(string asset)
    {
        if (string.IsNullOrWhiteSpace(asset)
            || Path.GetFileName(asset) != asset
            || asset.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0)
        {
            throw new InvalidDataException("Page asset name is invalid.");
        }

        var extension = Path.GetExtension(asset).ToLowerInvariant();
        if (extension is not (".jpg" or ".jpeg" or ".png"))
        {
            throw new InvalidDataException("Page asset type is unsupported.");
        }
    }
}

public sealed record LoadedProject(
    FastFillProject Project,
    string WorkspacePath);

public static class ProjectArchive
{
    private const long MaximumManifestBytes = 5L * 1024 * 1024;
    private const long MaximumArchiveContentBytes = 600L * 1024 * 1024;
    private const int MaximumEntries = 64;

    public static async Task SaveAsync(
        FastFillProject project,
        string workspacePath,
        string destinationPath,
        CancellationToken cancellationToken = default)
    {
        ProjectValidator.Validate(project);
        ArgumentException.ThrowIfNullOrWhiteSpace(workspacePath);
        ArgumentException.ThrowIfNullOrWhiteSpace(destinationPath);
        var destination = Path.GetFullPath(destinationPath);
        var destinationDirectory = Path.GetDirectoryName(destination)
            ?? throw new InvalidDataException("Destination has no directory.");
        Directory.CreateDirectory(destinationDirectory);
        var temporary = Path.Combine(
            destinationDirectory,
            $".{Path.GetFileName(destination)}.{Guid.NewGuid():N}.tmp");
        try
        {
            var assets = new List<(DocumentPage Page, string Path)>();
            long totalAssetBytes = 0;
            foreach (var page in project.Pages)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var source = AssetPath(workspacePath, page.SourceAsset);
                var sourceInfo = new FileInfo(source);
                if (!sourceInfo.Exists
                    || sourceInfo.Length
                        > ProjectValidator.MaximumImageBytes)
                {
                    throw new InvalidDataException(
                        $"Page asset is missing or too large: "
                        + page.SourceAsset);
                }

                totalAssetBytes = checked(
                    totalAssetBytes + sourceInfo.Length);
                if (totalAssetBytes > MaximumArchiveContentBytes)
                {
                    throw new InvalidDataException(
                        "Expanded project is too large.");
                }

                ProjectValidator.ValidateImageAsset(source, page);
                assets.Add((page, source));
            }

            await using (var stream = new FileStream(
                temporary,
                FileMode.CreateNew,
                FileAccess.ReadWrite,
                FileShare.None,
                64 * 1024,
                FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                using var archive = new ZipArchive(
                    stream,
                    ZipArchiveMode.Create,
                    leaveOpen: true);
                var manifest = archive.CreateEntry(
                    "project.json",
                    CompressionLevel.Optimal);
                await using (var manifestStream = manifest.Open())
                {
                    await JsonSerializer.SerializeAsync(
                        manifestStream,
                        project,
                        ProjectJson.Options,
                        cancellationToken);
                }

                foreach (var asset in assets)
                {
                    cancellationToken.ThrowIfCancellationRequested();
                    var entry = archive.CreateEntry(
                        $"assets/{asset.Page.SourceAsset}",
                        CompressionLevel.NoCompression);
                    await using var input = File.OpenRead(asset.Path);
                    await using var output = entry.Open();
                    await input.CopyToAsync(output, cancellationToken);
                }

                stream.Flush(flushToDisk: true);
            }

            await using (var stream = File.OpenRead(temporary))
            using (var archive = new ZipArchive(
                stream,
                ZipArchiveMode.Read,
                leaveOpen: false))
            {
                _ = ValidateArchive(archive);
            }

            File.Move(temporary, destination, overwrite: true);
        }
        catch
        {
            if (File.Exists(temporary))
            {
                File.Delete(temporary);
            }

            throw;
        }
    }

    public static async Task<LoadedProject> LoadAsync(
        string archivePath,
        string workspacePath,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(archivePath);
        ArgumentException.ThrowIfNullOrWhiteSpace(workspacePath);
        await using var stream = File.OpenRead(archivePath);
        using var archive = new ZipArchive(
            stream,
            ZipArchiveMode.Read,
            leaveOpen: false);
        var manifest = ValidateArchive(archive);

        FastFillProject project;
        await using (var manifestStream = manifest.Open())
        {
            project = await JsonSerializer.DeserializeAsync<FastFillProject>(
                manifestStream,
                ProjectJson.Options,
                cancellationToken)
                ?? throw new InvalidDataException("Project manifest is empty.");
        }

        ProjectValidator.Validate(project);
        if (Directory.Exists(workspacePath))
        {
            throw new IOException("Workspace already exists.");
        }

        var temporaryWorkspace = workspacePath + $".{Guid.NewGuid():N}.tmp";
        try
        {
            Directory.CreateDirectory(
                Path.Combine(temporaryWorkspace, "assets"));
            foreach (var page in project.Pages)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var entry = archive.GetEntry($"assets/{page.SourceAsset}")
                    ?? throw new InvalidDataException(
                        $"Page asset is missing: {page.SourceAsset}");
                if (entry.Length > ProjectValidator.MaximumImageBytes)
                {
                    throw new InvalidDataException("Page asset is too large.");
                }

                var destination = AssetPath(
                    temporaryWorkspace,
                    page.SourceAsset);
                await using var input = entry.Open();
                await using (var output = new FileStream(
                    destination,
                    FileMode.CreateNew,
                    FileAccess.Write,
                    FileShare.None,
                    64 * 1024,
                    FileOptions.Asynchronous))
                {
                    await input.CopyToAsync(output, cancellationToken);
                }

                ProjectValidator.ValidateImageAsset(destination, page);
            }

            await WorkspaceStore.SaveManifestAsync(
                project,
                temporaryWorkspace,
                cancellationToken);
            Directory.Move(temporaryWorkspace, workspacePath);
            return new(project, workspacePath);
        }
        catch
        {
            if (Directory.Exists(temporaryWorkspace))
            {
                Directory.Delete(temporaryWorkspace, recursive: true);
            }

            throw;
        }
    }

    private static ZipArchiveEntry ValidateArchive(ZipArchive archive)
    {
        if (archive.Entries.Count > MaximumEntries)
        {
            throw new InvalidDataException("Project has too many files.");
        }

        long totalLength = 0;
        foreach (var entry in archive.Entries)
        {
            totalLength = checked(totalLength + entry.Length);
            if (totalLength > MaximumArchiveContentBytes)
            {
                throw new InvalidDataException(
                    "Expanded project is too large.");
            }

            var name = entry.FullName.Replace('\\', '/');
            var valid = name == "project.json"
                || name.StartsWith("assets/", StringComparison.Ordinal)
                && name.Count(character => character == '/') == 1
                && Path.GetFileName(name) is { Length: > 0 } fileName
                && !fileName.Contains("..", StringComparison.Ordinal);
            if (!valid)
            {
                throw new InvalidDataException(
                    $"Unsafe project entry: {entry.FullName}");
            }
        }

        var manifest = archive.GetEntry("project.json")
            ?? throw new InvalidDataException("Project manifest is missing.");
        if (manifest.Length > MaximumManifestBytes)
        {
            throw new InvalidDataException("Project manifest is too large.");
        }

        return manifest;
    }

    private static string AssetPath(string workspace, string asset) =>
        Path.Combine(workspace, "assets", asset);
}

public static class WorkspaceStore
{
    public static string Create(string root, Guid projectId)
    {
        var path = Path.Combine(root, projectId.ToString("N"));
        Directory.CreateDirectory(Path.Combine(path, "assets"));
        return path;
    }

    public static async Task SaveManifestAsync(
        FastFillProject project,
        string workspacePath,
        CancellationToken cancellationToken = default)
    {
        ProjectValidator.Validate(project);
        Directory.CreateDirectory(workspacePath);
        var destination = Path.Combine(workspacePath, "project.json");
        var temporary = destination + $".{Guid.NewGuid():N}.tmp";
        try
        {
            await using (var stream = new FileStream(
                temporary,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                16 * 1024,
                FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                await JsonSerializer.SerializeAsync(
                    stream,
                    project,
                    ProjectJson.Options,
                    cancellationToken);
                stream.Flush(flushToDisk: true);
            }

            File.Move(temporary, destination, overwrite: true);
        }
        catch
        {
            if (File.Exists(temporary))
            {
                File.Delete(temporary);
            }

            throw;
        }
    }

    public static async Task<ProcessedPage> ProcessPageAsync(
        string workspacePath,
        DocumentPage page,
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var source = await File.ReadAllBytesAsync(
            Path.Combine(workspacePath, "assets", page.SourceAsset),
            cancellationToken);
        return await Task.Run(
            () => ImageProcessor.Process(
                source,
                page.Crop,
                page.RotationQuarterTurns,
                page.Filter),
            cancellationToken);
    }
}
