using System.IO.Compression;
using System.Text.Json;
using System.Text.Json.Serialization;

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

        if (project.Pages.Count > FastFillProject.MaximumPages)
        {
            throw new InvalidDataException("A project may contain five pages.");
        }

        if (project.Title.Length > 200)
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

    private static void ValidatePage(
        DocumentPage page,
        HashSet<Guid> pageIds)
    {
        if (!pageIds.Add(page.Id))
        {
            throw new InvalidDataException("Page IDs must be unique.");
        }

        ValidateAssetName(page.SourceAsset);
        ValidateImageInput(
            Math.Max(1, page.SourcePixelWidth * (long)page.SourcePixelHeight),
            page.SourcePixelWidth,
            page.SourcePixelHeight);
        if (!page.Crop.IsConvex())
        {
            throw new InvalidDataException("Crop corners must be convex.");
        }

        if (page.Filter.Contrast is < -100 or > 100)
        {
            throw new InvalidDataException("Page contrast is invalid.");
        }

        if (page.Annotations.Count > MaximumAnnotationsPerPage)
        {
            throw new InvalidDataException("Page has too many annotations.");
        }

        var annotationIds = new HashSet<Guid>();
        foreach (var annotation in page.Annotations)
        {
            if (!annotationIds.Add(annotation.Id))
            {
                throw new InvalidDataException(
                    "Annotation IDs must be unique per page.");
            }

            if (annotation.StrokeWidth is < 0.25f or > 40)
            {
                throw new InvalidDataException(
                    "Annotation stroke width is invalid.");
            }

            switch (annotation)
            {
                case FreehandAnnotation ink
                    when ink.Points.Count > MaximumInkPoints:
                    throw new InvalidDataException(
                        "Freehand annotation has too many points.");
                case TextAnnotation text when text.Text.Length > 10_000:
                    throw new InvalidDataException(
                        "Text annotation is too long.");
                case TextAnnotation text
                    when text.FontSize is < 6 or > 144:
                    throw new InvalidDataException(
                        "Text annotation font size is invalid.");
                case TextAnnotation text
                    when string.IsNullOrWhiteSpace(text.FontFamily)
                    || text.FontFamily.Length > 100:
                    throw new InvalidDataException(
                        "Text annotation font family is invalid.");
                case TextAnnotation text
                    when !float.IsFinite(text.MaximumWidth)
                    || text.MaximumWidth is <= 0 or > 1:
                    throw new InvalidDataException(
                        "Text annotation maximum width is invalid.");
            }
        }
    }

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

                    var entry = archive.CreateEntry(
                        $"assets/{page.SourceAsset}",
                        CompressionLevel.NoCompression);
                    await using var input = File.OpenRead(source);
                    await using var output = entry.Open();
                    await input.CopyToAsync(output, cancellationToken);
                }

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
        ValidateArchiveEntries(archive);
        var manifest = archive.GetEntry("project.json")
            ?? throw new InvalidDataException("Project manifest is missing.");
        if (manifest.Length > MaximumManifestBytes)
        {
            throw new InvalidDataException("Project manifest is too large.");
        }

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
                await using var output = new FileStream(
                    destination,
                    FileMode.CreateNew,
                    FileAccess.Write,
                    FileShare.None,
                    64 * 1024,
                    FileOptions.Asynchronous);
                await input.CopyToAsync(output, cancellationToken);
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

    private static void ValidateArchiveEntries(ZipArchive archive)
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
