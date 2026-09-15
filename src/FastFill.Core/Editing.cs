using System.Numerics;

namespace FastFill.Core;

public sealed class UndoBuffer<T>
{
    private readonly Func<T, T> _clone;
    private readonly int _capacity;
    private readonly Stack<T> _undo = [];
    private readonly Stack<T> _redo = [];

    public UndoBuffer(Func<T, T> clone, int capacity = 200)
    {
        ArgumentNullException.ThrowIfNull(clone);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(capacity);
        _clone = clone;
        _capacity = capacity;
    }

    public bool CanUndo => _undo.Count > 0;
    public bool CanRedo => _redo.Count > 0;

    public void Checkpoint(T current)
    {
        _undo.Push(_clone(current));
        _redo.Clear();
        TrimOldest(_undo);
    }

    public bool TryUndo(T current, out T result)
    {
        if (!_undo.TryPop(out var previous))
        {
            result = current;
            return false;
        }

        _redo.Push(_clone(current));
        result = previous;
        return true;
    }

    public bool TryRedo(T current, out T result)
    {
        if (!_redo.TryPop(out var next))
        {
            result = current;
            return false;
        }

        _undo.Push(_clone(current));
        result = next;
        return true;
    }

    public void Clear()
    {
        _undo.Clear();
        _redo.Clear();
    }

    private void TrimOldest(Stack<T> stack)
    {
        if (stack.Count <= _capacity)
        {
            return;
        }

        var values = stack.Take(_capacity).Reverse().ToArray();
        stack.Clear();
        foreach (var value in values)
        {
            stack.Push(value);
        }
    }
}

public static class AnnotationHitTester
{
    public static Annotation? HitTest(
        IReadOnlyList<Annotation> annotations,
        NormalizedPoint pagePoint,
        float tolerance = 0.015f)
    {
        for (var index = annotations.Count - 1; index >= 0; index--)
        {
            var annotation = annotations[index];
            if (!Matrix3x2.Invert(annotation.Transform.Matrix, out var inverse))
            {
                continue;
            }

            var local = NormalizedPoint.From(
                Vector2.Transform(pagePoint.Vector, inverse));
            if (Contains(annotation, local, tolerance))
            {
                return annotation;
            }
        }

        return null;
    }

    private static bool Contains(
        Annotation annotation,
        NormalizedPoint point,
        float tolerance) => annotation switch
    {
        FreehandAnnotation { Filled: true } ink =>
            PolygonContains(ink.Points, point)
            || PolylineDistance(ink.Points, point, close: true) <= tolerance,
        FreehandAnnotation ink =>
            PolylineDistance(ink.Points, point) <= tolerance,
        TextAnnotation text => text.Bounds.Contains(point, tolerance),
        ShapeAnnotation shape => Contains(shape, point, tolerance),
        _ => false,
    };

    private static bool Contains(
        ShapeAnnotation shape,
        NormalizedPoint point,
        float tolerance)
    {
        var bounds = NormalizedRect.FromPoints(shape.Start, shape.End);
        return shape.Shape switch
        {
            ShapeKind.Line or ShapeKind.Arrow =>
                SegmentDistance(point, shape.Start, shape.End) <= tolerance,
            ShapeKind.Rectangle when shape.Filled =>
                bounds.Contains(point, tolerance),
            ShapeKind.Rectangle => RectangleEdgeDistance(bounds, point)
                <= tolerance,
            ShapeKind.Ellipse => EllipseContains(
                bounds,
                point,
                tolerance,
                shape.Filled),
            ShapeKind.Checkmark => CheckmarkDistance(bounds, point)
                <= tolerance,
            ShapeKind.Cross => CrossDistance(bounds, point) <= tolerance,
            _ => false,
        };
    }

    private static float PolylineDistance(
        List<NormalizedPoint> points,
        NormalizedPoint point,
        bool close = false)
    {
        if (points.Count == 0)
        {
            return float.PositiveInfinity;
        }

        if (points.Count == 1)
        {
            return points[0].DistanceTo(point);
        }

        var distance = float.PositiveInfinity;
        for (var index = 1; index < points.Count; index++)
        {
            distance = Math.Min(
                distance,
                SegmentDistance(point, points[index - 1], points[index]));
        }

        if (close)
        {
            distance = Math.Min(
                distance,
                SegmentDistance(point, points[^1], points[0]));
        }

        return distance;
    }

    private static bool PolygonContains(
        List<NormalizedPoint> points,
        NormalizedPoint point)
    {
        if (points.Count < 3)
        {
            return false;
        }

        var inside = false;
        for (var index = 0; index < points.Count; index++)
        {
            var current = points[index];
            var previous = points[(index + points.Count - 1) % points.Count];
            if ((current.Y > point.Y) == (previous.Y > point.Y))
            {
                continue;
            }

            var crossing = (previous.X - current.X)
                * (point.Y - current.Y)
                / (previous.Y - current.Y)
                + current.X;
            if (point.X < crossing)
            {
                inside = !inside;
            }
        }

        return inside;
    }

    private static float SegmentDistance(
        NormalizedPoint point,
        NormalizedPoint start,
        NormalizedPoint end)
    {
        var segment = end.Vector - start.Vector;
        var lengthSquared = segment.LengthSquared();
        if (lengthSquared == 0)
        {
            return point.DistanceTo(start);
        }

        var offset = point.Vector - start.Vector;
        var fraction = Math.Clamp(
            Vector2.Dot(offset, segment) / lengthSquared,
            0,
            1);
        return Vector2.Distance(
            point.Vector,
            start.Vector + segment * fraction);
    }

    private static float RectangleEdgeDistance(
        NormalizedRect bounds,
        NormalizedPoint point)
    {
        if (!bounds.Contains(point))
        {
            return float.PositiveInfinity;
        }

        return new[]
        {
            Math.Abs(point.X - bounds.X),
            Math.Abs(point.X - bounds.Right),
            Math.Abs(point.Y - bounds.Y),
            Math.Abs(point.Y - bounds.Bottom),
        }.Min();
    }

    private static bool EllipseContains(
        NormalizedRect bounds,
        NormalizedPoint point,
        float tolerance,
        bool filled)
    {
        var radiusX = Math.Max(bounds.Width / 2, float.Epsilon);
        var radiusY = Math.Max(bounds.Height / 2, float.Epsilon);
        var centerX = bounds.X + radiusX;
        var centerY = bounds.Y + radiusY;
        var normalizedX = (point.X - centerX) / radiusX;
        var normalizedY = (point.Y - centerY) / radiusY;
        var radius = MathF.Sqrt(
            normalizedX * normalizedX + normalizedY * normalizedY);
        if (filled)
        {
            return radius <= 1 + (tolerance / Math.Min(radiusX, radiusY));
        }

        var normalizedTolerance = tolerance / Math.Min(radiusX, radiusY);
        return Math.Abs(radius - 1) <= normalizedTolerance;
    }

    private static float CheckmarkDistance(
        NormalizedRect bounds,
        NormalizedPoint point)
    {
        var start = new NormalizedPoint(
            bounds.X,
            bounds.Y + bounds.Height * 0.55f);
        var middle = new NormalizedPoint(
            bounds.X + bounds.Width * 0.38f,
            bounds.Bottom);
        var end = new NormalizedPoint(bounds.Right, bounds.Y);
        return Math.Min(
            SegmentDistance(point, start, middle),
            SegmentDistance(point, middle, end));
    }

    private static float CrossDistance(
        NormalizedRect bounds,
        NormalizedPoint point) => Math.Min(
        SegmentDistance(
            point,
            new NormalizedPoint(bounds.X, bounds.Y),
            new NormalizedPoint(bounds.Right, bounds.Bottom)),
        SegmentDistance(
            point,
            new NormalizedPoint(bounds.Right, bounds.Y),
            new NormalizedPoint(bounds.X, bounds.Bottom)));
}
