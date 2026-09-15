using System.Numerics;
using System.Text.Json.Serialization;

namespace FastFill.Core;

public readonly record struct NormalizedPoint(float X, float Y)
{
    public static NormalizedPoint From(Vector2 value) =>
        new(value.X, value.Y);

    public Vector2 Vector => new(X, Y);

    public NormalizedPoint Clamp() =>
        new(Math.Clamp(X, 0, 1), Math.Clamp(Y, 0, 1));

    public float DistanceTo(NormalizedPoint other) =>
        Vector2.Distance(Vector, other.Vector);
}

public readonly record struct NormalizedRect(
    float X,
    float Y,
    float Width,
    float Height)
{
    public float Right => X + Width;
    public float Bottom => Y + Height;

    public bool Contains(NormalizedPoint point, float tolerance = 0) =>
        point.X >= X - tolerance && point.X <= Right + tolerance
        && point.Y >= Y - tolerance && point.Y <= Bottom + tolerance;

    public static NormalizedRect FromPoints(
        NormalizedPoint first,
        NormalizedPoint second)
    {
        var left = Math.Min(first.X, second.X);
        var top = Math.Min(first.Y, second.Y);
        return new(
            left,
            top,
            Math.Abs(first.X - second.X),
            Math.Abs(first.Y - second.Y));
    }
}

public sealed record CropQuad(
    NormalizedPoint TopLeft,
    NormalizedPoint TopRight,
    NormalizedPoint BottomRight,
    NormalizedPoint BottomLeft)
{
    public static CropQuad Full { get; } = new(
        new(0, 0),
        new(1, 0),
        new(1, 1),
        new(0, 1));

    [JsonIgnore]
    public IReadOnlyList<NormalizedPoint> Points =>
        [TopLeft, TopRight, BottomRight, BottomLeft];

    public bool IsConvex()
    {
        var signs = new float[4];
        for (var index = 0; index < 4; index++)
        {
            var a = Points[index].Vector;
            var b = Points[(index + 1) % 4].Vector;
            var c = Points[(index + 2) % 4].Vector;
            signs[index] = Cross(b - a, c - b);
        }

        return signs.All(value => value > 0)
            || signs.All(value => value < 0);
    }

    public CropQuad Clamp() => new(
        TopLeft.Clamp(),
        TopRight.Clamp(),
        BottomRight.Clamp(),
        BottomLeft.Clamp());

    public float MaximumCornerDistance(CropQuad other) =>
        Points.Zip(other.Points)
            .Max(pair => pair.First.DistanceTo(pair.Second));

    private static float Cross(Vector2 first, Vector2 second) =>
        (first.X * second.Y) - (first.Y * second.X);
}

public enum PageFilterMode
{
    Original,
    EnhancedColor,
    Grayscale,
    BlackAndWhite,
}

public sealed record PageFilterSettings
{
    public PageFilterMode Mode { get; init; } = PageFilterMode.Original;
    public int Contrast { get; init; }
}

public readonly record struct AffineTransform(
    float M11,
    float M12,
    float M21,
    float M22,
    float M31,
    float M32)
{
    public static AffineTransform Identity { get; } =
        new(1, 0, 0, 1, 0, 0);

    public NormalizedPoint Apply(NormalizedPoint point) =>
        NormalizedPoint.From(Vector2.Transform(point.Vector, Matrix));

    [JsonIgnore]
    public Matrix3x2 Matrix =>
        new(M11, M12, M21, M22, M31, M32);

    public static AffineTransform FromMatrix(Matrix3x2 matrix) =>
        new(
            matrix.M11,
            matrix.M12,
            matrix.M21,
            matrix.M22,
            matrix.M31,
            matrix.M32);
}

public enum ShapeKind
{
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    Checkmark,
    Cross,
}

[JsonPolymorphic(TypeDiscriminatorPropertyName = "kind")]
[JsonDerivedType(typeof(FreehandAnnotation), "freehand")]
[JsonDerivedType(typeof(TextAnnotation), "text")]
[JsonDerivedType(typeof(ShapeAnnotation), "shape")]
public abstract record Annotation
{
    public Guid Id { get; init; } = Guid.NewGuid();
    public uint ColorArgb { get; init; } = 0xff111827;
    public float StrokeWidth { get; init; } = 3;
    public AffineTransform Transform { get; init; } =
        AffineTransform.Identity;
}

public sealed record FreehandAnnotation : Annotation
{
    public List<NormalizedPoint> Points { get; init; } = [];
    public bool IsHighlighter { get; init; }
    public bool Filled { get; init; }
}

public sealed record TextAnnotation : Annotation
{
    public string Text { get; init; } = string.Empty;
    public NormalizedRect Bounds { get; init; } =
        new(0.1f, 0.1f, 0.3f, 0.1f);
    public float FontSize { get; init; } = 14;
    public string FontFamily { get; init; } = "Segoe UI";
    public float MaximumWidth { get; init; } = 0.4f;
}

public sealed record ShapeAnnotation : Annotation
{
    public ShapeKind Shape { get; init; }
    public NormalizedPoint Start { get; init; }
    public NormalizedPoint End { get; init; }
    public bool Filled { get; init; }
}

public sealed record DocumentPage
{
    public Guid Id { get; init; } = Guid.NewGuid();
    public string SourceAsset { get; init; } = string.Empty;
    public int SourcePixelWidth { get; init; }
    public int SourcePixelHeight { get; init; }
    public CropQuad Crop { get; init; } = CropQuad.Full;
    public int RotationQuarterTurns { get; init; }
    public PageFilterSettings Filter { get; init; } = new();
    public List<Annotation> Annotations { get; init; } = [];
}

public sealed record FastFillProject
{
    public const int CurrentSchemaVersion = 1;
    public const int MaximumPages = 5;

    public int SchemaVersion { get; init; } = CurrentSchemaVersion;
    public Guid Id { get; init; } = Guid.NewGuid();
    public string Title { get; init; } = "Untitled scan";
    public DateTimeOffset CreatedUtc { get; init; } = DateTimeOffset.UtcNow;
    public DateTimeOffset UpdatedUtc { get; init; } = DateTimeOffset.UtcNow;
    public List<DocumentPage> Pages { get; init; } = [];
}
