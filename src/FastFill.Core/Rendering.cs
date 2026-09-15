using System.Numerics;
using SkiaSharp;

namespace FastFill.Core;

public sealed record RenderOptions(
    float PageWidthPoints,
    float PageHeightPoints,
    Guid? SelectedAnnotationId = null,
    bool DrawSelection = false,
    Guid? HiddenAnnotationId = null);

public static class PageRenderer
{
    public static void Draw(
        SKCanvas canvas,
        SKImage background,
        DocumentPage page,
        SKRect destination,
        RenderOptions options)
    {
        ArgumentNullException.ThrowIfNull(canvas);
        ArgumentNullException.ThrowIfNull(background);
        ArgumentNullException.ThrowIfNull(page);
        ArgumentNullException.ThrowIfNull(options);
        canvas.DrawImage(
            background,
            destination,
            new SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.None));
        canvas.Save();
        canvas.Translate(destination.Left, destination.Top);
        canvas.Scale(destination.Width, destination.Height);
        foreach (var annotation in page.Annotations)
        {
            if (annotation.Id == options.HiddenAnnotationId)
            {
                continue;
            }

            DrawAnnotation(canvas, annotation, options);
        }

        canvas.Restore();
    }

    public static byte[] RenderPng(
        ProcessedPage background,
        DocumentPage page,
        int maximumEdge = 1400)
    {
        using var image = SKImage.FromEncodedData(background.EncodedPng)
            ?? throw new InvalidOperationException("Page image is invalid.");
        var scale = Math.Min(
            1,
            maximumEdge / (double)Math.Max(image.Width, image.Height));
        var width = Math.Max(1, (int)Math.Round(image.Width * scale));
        var height = Math.Max(1, (int)Math.Round(image.Height * scale));
        var info = new SKImageInfo(
            width,
            height,
            SKColorType.Bgra8888,
            SKAlphaType.Premul,
            SKColorSpace.CreateSrgb());
        using var surface = SKSurface.Create(info)
            ?? throw new InvalidOperationException("Render surface failed.");
        surface.Canvas.Clear(SKColors.White);
        Draw(
            surface.Canvas,
            image,
            page,
            new SKRect(0, 0, width, height),
            new RenderOptions(612, 612f * height / width));
        using var snapshot = surface.Snapshot();
        using var data = snapshot.Encode(SKEncodedImageFormat.Png, 100);
        return data.ToArray();
    }

    private static void DrawAnnotation(
        SKCanvas canvas,
        Annotation annotation,
        RenderOptions options)
    {
        canvas.Save();
        canvas.Concat(ToSkia(annotation.Transform));
        using var paint = CreatePaint(annotation, options);
        switch (annotation)
        {
            case FreehandAnnotation freehand:
                DrawFreehand(canvas, freehand, paint);
                break;
            case TextAnnotation text:
                DrawText(canvas, text, paint, options);
                break;
            case ShapeAnnotation shape:
                DrawShape(canvas, shape, paint, options);
                break;
        }

        if (options.DrawSelection
            && options.SelectedAnnotationId == annotation.Id)
        {
            DrawSelection(canvas, annotation, options);
        }

        canvas.Restore();
    }

    private static SKPaint CreatePaint(
        Annotation annotation,
        RenderOptions options)
    {
        var color = new SKColor(annotation.ColorArgb);
        if (annotation is FreehandAnnotation { IsHighlighter: true })
        {
            color = color.WithAlpha(89);
        }

        return new SKPaint
        {
            Color = color,
            IsAntialias = true,
            Style = SKPaintStyle.Stroke,
            StrokeCap = SKStrokeCap.Round,
            StrokeJoin = SKStrokeJoin.Round,
            StrokeWidth = annotation.StrokeWidth / options.PageWidthPoints,
        };
    }

    private static void DrawFreehand(
        SKCanvas canvas,
        FreehandAnnotation annotation,
        SKPaint paint)
    {
        if (annotation.Points.Count == 0)
        {
            return;
        }

        using var builder = new SKPathBuilder();
        builder.MoveTo(annotation.Points[0].X, annotation.Points[0].Y);
        foreach (var point in annotation.Points.Skip(1))
        {
            builder.LineTo(point.X, point.Y);
        }

        if (annotation.Filled && annotation.Points.Count >= 3)
        {
            builder.Close();
        }

        using var path = builder.Detach();
        if (annotation.Filled && annotation.Points.Count >= 3)
        {
            paint.Style = SKPaintStyle.Fill;
            canvas.DrawPath(path, paint);
            paint.Style = SKPaintStyle.Stroke;
        }

        canvas.DrawPath(path, paint);
    }

    private static void DrawText(
        SKCanvas canvas,
        TextAnnotation annotation,
        SKPaint paint,
        RenderOptions options)
    {
        paint.Style = SKPaintStyle.Fill;
        using var typeface = SKTypeface.FromFamilyName(
            annotation.FontFamily);
        using var font = new SKFont(
            typeface ?? SKTypeface.Default,
            annotation.FontSize / options.PageWidthPoints);
        var lineHeight = font.Size * 1.25f;
        var y = annotation.Bounds.Y + font.Size;
        foreach (var line in WrapText(annotation.Text, font, annotation.Bounds))
        {
            if (y > annotation.Bounds.Bottom)
            {
                break;
            }

            canvas.DrawText(
                line,
                annotation.Bounds.X,
                y,
                SKTextAlign.Left,
                font,
                paint);
            y += lineHeight;
        }
    }

    private static IEnumerable<string> WrapText(
        string text,
        SKFont font,
        NormalizedRect bounds)
    {
        foreach (var paragraph in text.ReplaceLineEndings("\n").Split('\n'))
        {
            var line = string.Empty;
            foreach (var word in paragraph.Split(' '))
            {
                var candidate = line.Length == 0 ? word : $"{line} {word}";
                if (line.Length > 0
                    && font.MeasureText(candidate) > bounds.Width)
                {
                    yield return line;
                    line = word;
                }
                else
                {
                    line = candidate;
                }
            }

            yield return line;
        }
    }

    private static void DrawShape(
        SKCanvas canvas,
        ShapeAnnotation annotation,
        SKPaint paint,
        RenderOptions options)
    {
        var bounds = ToSkia(
            NormalizedRect.FromPoints(annotation.Start, annotation.End));
        if (annotation.Filled)
        {
            paint.Style = SKPaintStyle.Fill;
        }

        switch (annotation.Shape)
        {
            case ShapeKind.Line:
                DrawLine(canvas, annotation, paint);
                break;
            case ShapeKind.Arrow:
                DrawArrow(canvas, annotation, paint, options);
                break;
            case ShapeKind.Rectangle:
                canvas.DrawRect(bounds, paint);
                break;
            case ShapeKind.Ellipse:
                canvas.DrawOval(bounds, paint);
                break;
            case ShapeKind.Checkmark:
                DrawCheckmark(canvas, bounds, paint);
                break;
            case ShapeKind.Cross:
                DrawCross(canvas, bounds, paint);
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(annotation));
        }
    }

    private static void DrawLine(
        SKCanvas canvas,
        ShapeAnnotation annotation,
        SKPaint paint) => canvas.DrawLine(
            annotation.Start.X,
            annotation.Start.Y,
            annotation.End.X,
            annotation.End.Y,
            paint);

    private static void DrawArrow(
        SKCanvas canvas,
        ShapeAnnotation annotation,
        SKPaint paint,
        RenderOptions options)
    {
        DrawLine(canvas, annotation, paint);
        var width = options.PageWidthPoints;
        var height = options.PageHeightPoints;
        var start = new Vector2(
            annotation.Start.X * width,
            annotation.Start.Y * height);
        var end = new Vector2(
            annotation.End.X * width,
            annotation.End.Y * height);
        var direction = end - start;
        if (direction.LengthSquared() < float.Epsilon)
        {
            return;
        }

        direction = Vector2.Normalize(direction);
        var normal = new Vector2(-direction.Y, direction.X);
        var size = Math.Max(10, annotation.StrokeWidth * 4);
        var left = end - direction * size + normal * size * 0.55f;
        var right = end - direction * size - normal * size * 0.55f;
        canvas.DrawLine(
            end.X / width,
            end.Y / height,
            left.X / width,
            left.Y / height,
            paint);
        canvas.DrawLine(
            end.X / width,
            end.Y / height,
            right.X / width,
            right.Y / height,
            paint);
    }

    private static void DrawCheckmark(
        SKCanvas canvas,
        SKRect bounds,
        SKPaint paint)
    {
        paint.Style = SKPaintStyle.Stroke;
        using var builder = new SKPathBuilder();
        builder.MoveTo(bounds.Left, bounds.Top + bounds.Height * 0.55f);
        builder.LineTo(
            bounds.Left + bounds.Width * 0.38f,
            bounds.Bottom);
        builder.LineTo(bounds.Right, bounds.Top);
        using var path = builder.Detach();
        canvas.DrawPath(path, paint);
    }

    private static void DrawCross(
        SKCanvas canvas,
        SKRect bounds,
        SKPaint paint)
    {
        paint.Style = SKPaintStyle.Stroke;
        canvas.DrawLine(
            bounds.Left,
            bounds.Top,
            bounds.Right,
            bounds.Bottom,
            paint);
        canvas.DrawLine(
            bounds.Right,
            bounds.Top,
            bounds.Left,
            bounds.Bottom,
            paint);
    }

    private static void DrawSelection(
        SKCanvas canvas,
        Annotation annotation,
        RenderOptions options)
    {
        var bounds = GetBounds(annotation);
        using var selection = new SKPaint
        {
            Color = SKColors.DeepSkyBlue,
            IsAntialias = true,
            Style = SKPaintStyle.Stroke,
            StrokeWidth = 1.5f / options.PageWidthPoints,
        };
        canvas.DrawRect(ToSkia(bounds), selection);
        var radiusX = 7 / options.PageWidthPoints;
        var radiusY = 7 / options.PageHeightPoints;
        using var handleFill = new SKPaint
        {
            Color = SKColors.White,
            IsAntialias = true,
            Style = SKPaintStyle.Fill,
        };
        foreach (var point in BoundsCorners(bounds))
        {
            var handle = new SKRect(
                point.X - radiusX,
                point.Y - radiusY,
                point.X + radiusX,
                point.Y + radiusY);
            canvas.DrawOval(handle, handleFill);
            canvas.DrawOval(handle, selection);
        }
    }

    private static NormalizedPoint[] BoundsCorners(NormalizedRect bounds) =>
    [
        new(bounds.X, bounds.Y),
        new(bounds.Right, bounds.Y),
        new(bounds.Right, bounds.Bottom),
        new(bounds.X, bounds.Bottom),
    ];

    private static NormalizedRect GetBounds(Annotation annotation) =>
        annotation switch
        {
            FreehandAnnotation ink when ink.Points.Count > 0 => new(
                ink.Points.Min(point => point.X),
                ink.Points.Min(point => point.Y),
                ink.Points.Max(point => point.X)
                    - ink.Points.Min(point => point.X),
                ink.Points.Max(point => point.Y)
                    - ink.Points.Min(point => point.Y)),
            TextAnnotation text => text.Bounds,
            ShapeAnnotation shape =>
                NormalizedRect.FromPoints(shape.Start, shape.End),
            _ => new(0, 0, 0, 0),
        };

    private static SKRect ToSkia(NormalizedRect rectangle) =>
        new(rectangle.X, rectangle.Y, rectangle.Right, rectangle.Bottom);

    private static SKMatrix ToSkia(AffineTransform transform) => new()
    {
        ScaleX = transform.M11,
        SkewX = transform.M21,
        TransX = transform.M31,
        SkewY = transform.M12,
        ScaleY = transform.M22,
        TransY = transform.M32,
        Persp0 = 0,
        Persp1 = 0,
        Persp2 = 1,
    };
}

public static class PdfExporter
{
    public const float DotsPerInch = 300;

    public static async Task ExportAsync(
        FastFillProject project,
        Func<DocumentPage, CancellationToken, Task<ProcessedPage>> process,
        Stream output,
        IProgress<int>? progress = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(project);
        ArgumentNullException.ThrowIfNull(process);
        ArgumentNullException.ThrowIfNull(output);
        ProjectValidator.Validate(project);
        var metadata = new SKDocumentPdfMetadata
        {
            Title = project.Title,
            Author = string.Empty,
            Subject = "Scanned document",
            Keywords = "FastFill, scan",
            Creator = "FastFill",
            Producer = "FastFill",
            Creation = project.CreatedUtc.UtcDateTime,
            Modified = project.UpdatedUtc.UtcDateTime,
            RasterDpi = DotsPerInch,
            EncodingQuality = 90,
        };
        using var document = SKDocument.CreatePdf(output, metadata)
            ?? throw new InvalidOperationException("PDF creation failed.");
        for (var index = 0; index < project.Pages.Count; index++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var page = project.Pages[index];
            var processed = await process(page, cancellationToken);
            using var image = SKImage.FromEncodedData(processed.EncodedPng)
                ?? throw new InvalidOperationException(
                    "Page image is invalid.");
            var widthPoints = processed.Width / DotsPerInch * 72;
            var heightPoints = processed.Height / DotsPerInch * 72;
            using var canvas = document.BeginPage(widthPoints, heightPoints);
            PageRenderer.Draw(
                canvas,
                image,
                page,
                new SKRect(0, 0, widthPoints, heightPoints),
                new RenderOptions(widthPoints, heightPoints));
            document.EndPage();
            progress?.Report(index + 1);
        }

        document.Close();
        await output.FlushAsync(cancellationToken);
    }
}
