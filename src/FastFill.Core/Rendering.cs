using System.Numerics;
using SkiaSharp;

namespace FastFill.Core;

public sealed record RenderOptions(
    float PageWidthPoints,
    float PageHeightPoints,
    IReadOnlySet<Guid>? SelectedAnnotationIds = null,
    bool DrawSelection = false,
    Guid? HiddenAnnotationId = null,
    Guid? TransformAnnotationId = null,
    NormalizedRect? SelectionRectangle = null);

public static class PageRenderer
{
    public const float RotationHandleOffset = 24;

    public static TextAnnotation FitTextBounds(
        TextAnnotation annotation,
        float pageWidth,
        float pageHeight)
    {
        ArgumentNullException.ThrowIfNull(annotation);
        ArgumentOutOfRangeException.ThrowIfLessThan(pageWidth, 1);
        ArgumentOutOfRangeException.ThrowIfLessThan(pageHeight, 1);
        using var typeface = SKTypeface.FromFamilyName(
            annotation.FontFamily);
        using var font = new SKFont(
            typeface ?? SKTypeface.Default,
            annotation.FontSize);
        var maximumWidth = Math.Clamp(
            annotation.MaximumWidth * pageWidth,
            font.Size,
            pageWidth);
        var lines = WrapText(annotation.Text, font, maximumWidth).ToArray();
        var contentWidth = lines.Length == 0
            ? font.Size
            : Math.Max(
                font.Size,
                lines.Max(line => MeasureText(line, font)) + 2);
        var width = Math.Min(maximumWidth, contentWidth) / pageWidth;
        var height = Math.Max(1, lines.Length)
            * font.Size
            * 1.25f
            / pageHeight;
        var x = Math.Clamp(
            annotation.Bounds.X,
            0,
            Math.Max(0, 1 - width));
        var y = Math.Clamp(
            annotation.Bounds.Y,
            0,
            Math.Max(0, 1 - height));
        return annotation with
        {
            Bounds = new NormalizedRect(x, y, width, height),
        };
    }

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
        canvas.Scale(
            destination.Width / options.PageWidthPoints,
            destination.Height / options.PageHeightPoints);
        var displayScale = destination.Width / options.PageWidthPoints;
        foreach (var annotation in page.Annotations)
        {
            if (annotation.Id == options.HiddenAnnotationId)
            {
                continue;
            }

            DrawAnnotation(canvas, annotation, options, displayScale);
        }

        if (options.SelectionRectangle is NormalizedRect selectionRectangle)
        {
            DrawSelectionRectangle(
                canvas,
                selectionRectangle,
                options,
                displayScale);
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
        RenderOptions options,
        float displayScale)
    {
        canvas.Save();
        canvas.Concat(ToSkia(annotation.Transform, options));
        using var paint = CreatePaint(annotation);
        switch (annotation)
        {
            case FreehandAnnotation freehand:
                DrawFreehand(canvas, freehand, paint, options);
                break;
            case TextAnnotation text:
                DrawText(canvas, text, paint, options);
                break;
            case ShapeAnnotation shape:
                DrawShape(canvas, shape, paint, options);
                break;
        }

        if (options.DrawSelection
            && options.SelectedAnnotationIds?.Contains(annotation.Id) == true)
        {
            DrawSelection(
                canvas,
                annotation,
                options,
                displayScale,
                annotation.Id == options.TransformAnnotationId);
        }

        canvas.Restore();
    }

    private static SKPaint CreatePaint(Annotation annotation)
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
            StrokeWidth = annotation.StrokeWidth,
        };
    }

    private static void DrawFreehand(
        SKCanvas canvas,
        FreehandAnnotation annotation,
        SKPaint paint,
        RenderOptions options)
    {
        if (annotation.Points.Count == 0)
        {
            return;
        }

        using var builder = new SKPathBuilder();
        builder.MoveTo(ToSkia(annotation.Points[0], options));
        foreach (var point in annotation.Points.Skip(1))
        {
            builder.LineTo(ToSkia(point, options));
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
            annotation.FontSize);
        var lineHeight = font.Size * 1.25f;
        var bounds = ToSkia(annotation.Bounds, options);
        var y = bounds.Top + font.Size;
        foreach (var line in WrapText(annotation.Text, font, bounds.Width))
        {
            if (y > bounds.Bottom)
            {
                break;
            }

            canvas.DrawText(
                line,
                bounds.Left,
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
        float width)
    {
        foreach (var paragraph in text.ReplaceLineEndings("\n").Split('\n'))
        {
            var line = string.Empty;
            foreach (var word in paragraph.Split(' '))
            {
                var candidate = line.Length == 0 ? word : $"{line} {word}";
                if (line.Length > 0
                    && MeasureText(candidate, font) > width)
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

    private static float MeasureText(string text, SKFont font)
    {
        var measured = font.MeasureText(text);
        // Headless build images can lack fonts. Keep wrapping deterministic.
        return measured > 0 || text.Length == 0
            ? measured
            : text.Length * font.Size * 0.55f;
    }

    private static void DrawShape(
        SKCanvas canvas,
        ShapeAnnotation annotation,
        SKPaint paint,
        RenderOptions options)
    {
        var bounds = ToSkia(
            NormalizedRect.FromPoints(annotation.Start, annotation.End),
            options);
        if (annotation.Filled
            && annotation.Shape is ShapeKind.Rectangle
                or ShapeKind.Ellipse)
        {
            paint.Style = SKPaintStyle.Fill;
        }

        switch (annotation.Shape)
        {
            case ShapeKind.Line:
                DrawLine(canvas, annotation, paint, options);
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
        SKPaint paint,
        RenderOptions options) => canvas.DrawLine(
            ToSkia(annotation.Start, options),
            ToSkia(annotation.End, options),
            paint);

    private static void DrawArrow(
        SKCanvas canvas,
        ShapeAnnotation annotation,
        SKPaint paint,
        RenderOptions options)
    {
        DrawLine(canvas, annotation, paint, options);
        var start = ToVector(annotation.Start, options);
        var end = ToVector(annotation.End, options);
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
            end.X,
            end.Y,
            left.X,
            left.Y,
            paint);
        canvas.DrawLine(
            end.X,
            end.Y,
            right.X,
            right.Y,
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
        RenderOptions options,
        float displayScale,
        bool drawHandles)
    {
        var bounds = AnnotationGeometry.Bounds(annotation);
        using var selection = new SKPaint
        {
            Color = SKColors.DeepSkyBlue,
            IsAntialias = true,
            Style = SKPaintStyle.Stroke,
            StrokeWidth = 1.5f / displayScale,
        };
        canvas.DrawRect(ToSkia(bounds, options), selection);
        if (!drawHandles)
        {
            return;
        }

        var radius = 7 / displayScale;
        using var handleFill = new SKPaint
        {
            Color = SKColors.White,
            IsAntialias = true,
            Style = SKPaintStyle.Fill,
        };
        foreach (var point in AnnotationGeometry.ResizeHandles(annotation))
        {
            var center = ToSkia(point, options);
            var handle = new SKRect(
                center.X - radius,
                center.Y - radius,
                center.X + radius,
                center.Y + radius);
            canvas.DrawOval(handle, handleFill);
            canvas.DrawOval(handle, selection);
        }

        var top = new SKPoint(
            (bounds.X + bounds.Right) / 2 * options.PageWidthPoints,
            bounds.Y * options.PageHeightPoints);
        var rotationCenter = new SKPoint(
            top.X,
            top.Y - RotationHandleOffset / displayScale);
        canvas.DrawLine(top, rotationCenter, selection);
        canvas.DrawCircle(rotationCenter, radius, handleFill);
        canvas.DrawCircle(rotationCenter, radius, selection);
    }

    private static void DrawSelectionRectangle(
        SKCanvas canvas,
        NormalizedRect rectangle,
        RenderOptions options,
        float displayScale)
    {
        using var paint = new SKPaint
        {
            Color = SKColors.DeepSkyBlue,
            IsAntialias = true,
            Style = SKPaintStyle.Stroke,
            StrokeWidth = 1.5f / displayScale,
            PathEffect = SKPathEffect.CreateDash(
                [6 / displayScale, 4 / displayScale],
                0),
        };
        canvas.DrawRect(ToSkia(rectangle, options), paint);
    }

    private static SKPoint ToSkia(
        NormalizedPoint point,
        RenderOptions options) => new(
        point.X * options.PageWidthPoints,
        point.Y * options.PageHeightPoints);

    private static Vector2 ToVector(
        NormalizedPoint point,
        RenderOptions options) => new(
        point.X * options.PageWidthPoints,
        point.Y * options.PageHeightPoints);

    private static SKRect ToSkia(
        NormalizedRect rectangle,
        RenderOptions options) => new(
        rectangle.X * options.PageWidthPoints,
        rectangle.Y * options.PageHeightPoints,
        rectangle.Right * options.PageWidthPoints,
        rectangle.Bottom * options.PageHeightPoints);

    private static SKMatrix ToSkia(
        AffineTransform transform,
        RenderOptions options) => new()
    {
        ScaleX = transform.M11,
        SkewX = transform.M21
            * options.PageWidthPoints
            / options.PageHeightPoints,
        TransX = transform.M31 * options.PageWidthPoints,
        SkewY = transform.M12
            * options.PageHeightPoints
            / options.PageWidthPoints,
        ScaleY = transform.M22,
        TransY = transform.M32 * options.PageHeightPoints,
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
