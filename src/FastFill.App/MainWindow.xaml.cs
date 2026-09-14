using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.Numerics;
using System.Runtime.InteropServices;
using System.Text.Json;
using FastFill.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using SkiaSharp;
using SkiaSharp.Views.Windows;
using Windows.ApplicationModel.DataTransfer;
using Windows.Foundation;
using Windows.Storage;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;
using Windows.System;
using WinRT;

namespace FastFill.App;

[SuppressMessage(
    "Design",
    "CA1001:Types that own disposable fields should be disposable",
    Justification = "WinUI owns Window lifetime; Closed releases resources.")]
public sealed partial class MainWindow : Window
{
    private static readonly Guid DataTransferManagerId = new(
        0xa5caee9b,
        0x8708,
        0x49d1,
        0x8d,
        0x36,
        0x67,
        0xd2,
        0x5a,
        0x8d,
        0xa0,
        0x0c);

    private readonly UndoBuffer<FastFillProject> _history =
        new(ProjectJson.Clone);
    private readonly AutoCaptureGate _autoCaptureGate = new();
    private readonly string _workspaceRoot;
    private FastFillProject _project = new();
    private string _workspace = string.Empty;
    private string? _projectPath;
    private IReadOnlyList<CameraChoice> _cameraChoices = [];
    private CameraFrameSource? _camera;
    private int _cameraIndex;
    private int _detecting;
    private int _capturing;
    private DocumentDetection? _latestDetection;
    private byte[]? _pendingBytes;
    private string _pendingExtension = ".jpg";
    private CropQuad _pendingCrop = CropQuad.Full;
    private PageFilterSettings _pendingFilter = new();
    private int _pendingRotation;
    private int? _editingPageIndex;
    private SKImage? _reviewSource;
    private SKImage? _reviewProcessed;
    private ProcessedPage? _processedPage;
    private SKImage? _editorBackground;
    private CancellationTokenSource? _renderCancellation;
    private CancellationTokenSource? _autosaveCancellation;
    private SKRect _reviewImageRect;
    private SKRect _editorImageRect;
    private int? _activeCropCorner;
    private int _pageIndex;
    private ToolMode _tool = ToolMode.Select;
    private uint _toolColor = 0xff111827;
    private NormalizedPoint? _pointerStart;
    private Guid? _selectedAnnotationId;
    private AffineTransform? _dragStartTransform;
    private Guid? _draftAnnotationId;
    private float _zoom = 1;
    private bool _dirty;
    private bool _updatingPageList;
    private DataTransferManager? _dataTransferManager;
    private StorageFile? _shareFile;
    private AppScreen _screen = AppScreen.Home;

    public MainWindow()
    {
        InitializeComponent();
        _workspaceRoot = Path.Combine(
            ApplicationData.Current.LocalFolder.Path,
            "FastFill",
            "workspaces");
        Directory.CreateDirectory(_workspaceRoot);
        InitializeShare();
        InstallKeyboardShortcuts();
        Closed += MainWindow_Closed;
        VisibilityChanged += MainWindow_VisibilityChanged;
        ShowRecoverIfAvailable();
    }

    private async void NewScanButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        CreateProject();
        await StartCameraAsync();
    }

    private async void AddPageButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (!CanAddPage())
        {
            return;
        }

        _editingPageIndex = null;
        await StartCameraAsync();
    }

    private async void ImportButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        try
        {
            if (string.IsNullOrEmpty(_workspace))
            {
                CreateProject();
            }

            if (!CanAddPage())
            {
                return;
            }

            var picker = new FileOpenPicker();
            InitializePicker(picker);
            picker.FileTypeFilter.Add(".jpg");
            picker.FileTypeFilter.Add(".jpeg");
            picker.FileTypeFilter.Add(".png");
            var file = await picker.PickSingleFileAsync();
            if (file is null)
            {
                return;
            }

            var bytes = await ReadStorageFileAsync(file);
            _editingPageIndex = null;
            await BeginReviewAsync(bytes, file.FileType);
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Import failed", exception);
        }
    }

    private async void OpenButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        try
        {
            var picker = new FileOpenPicker();
            InitializePicker(picker);
            picker.FileTypeFilter.Add(".docscan");
            var file = await picker.PickSingleFileAsync();
            if (file is null)
            {
                return;
            }

            var workspace = Path.Combine(
                _workspaceRoot,
                Guid.NewGuid().ToString("N"));
            var loaded = await ProjectArchive.LoadAsync(file.Path, workspace);
            _project = loaded.Project;
            _workspace = loaded.WorkspacePath;
            _projectPath = file.Path;
            _pageIndex = 0;
            _history.Clear();
            _dirty = false;
            ShowEditor();
            await LoadActivePageAsync();
            SetStatus($"Opened {_project.Title}.");
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Open failed", exception);
        }
    }

    private async void RecoverButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        try
        {
            var manifest = FindLatestRecoveryManifest();
            if (manifest is null)
            {
                RecoverButton.Visibility = Visibility.Collapsed;
                return;
            }

            await using var stream = File.OpenRead(manifest);
            var project = await JsonSerializer
                .DeserializeAsync<FastFillProject>(
                    stream,
                    ProjectJson.Options)
                ?? throw new InvalidDataException(
                    "Recovery manifest is empty.");
            ProjectValidator.Validate(project);
            _project = project;
            _workspace = Path.GetDirectoryName(manifest)
                ?? throw new InvalidDataException(
                    "Recovery path is invalid.");
            _projectPath = null;
            _pageIndex = 0;
            _dirty = true;
            ShowEditor();
            await LoadActivePageAsync();
            SetStatus("Recovered autosaved project.");
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Recovery failed", exception);
        }
    }

    private void CreateProject()
    {
        _project = new FastFillProject
        {
            Title = $"Scan {DateTime.Now:yyyy-MM-dd HH-mm}",
        };
        _workspace = WorkspaceStore.Create(_workspaceRoot, _project.Id);
        _projectPath = null;
        _pageIndex = 0;
        _history.Clear();
        _dirty = true;
        UpdateCommandState();
    }

    private bool CanAddPage()
    {
        if (_project.Pages.Count < FastFillProject.MaximumPages)
        {
            return true;
        }

        SetStatus("FastFill v1 supports up to five pages.");
        return false;
    }

    private async Task StartCameraAsync()
    {
        try
        {
            SetScreen(AppScreen.Capture);
            CaptureStatusText.Text = "Starting camera…";
            await StopCameraAsync();
            if (_cameraChoices.Count == 0)
            {
                _cameraChoices = await CameraFrameSource.FindAsync();
                _cameraIndex = Math.Max(
                    0,
                    _cameraChoices.ToList().FindIndex(choice =>
                        choice.Position == CameraPosition.Back));
            }

            if (_cameraChoices.Count == 0)
            {
                throw new InvalidOperationException("No camera was found.");
            }

            var choice = _cameraChoices[_cameraIndex];
            CameraPreview.RenderTransformOrigin = new Point(0.5, 0.5);
            CameraPreview.RenderTransform = new ScaleTransform
            {
                ScaleX = choice.Position == CameraPosition.Front ? -1 : 1,
                ScaleY = 1,
            };
            _camera = new CameraFrameSource(choice, CameraPreview);
            _camera.FrameArrived += Camera_FrameArrived;
            _camera.Failed += Camera_Failed;
            await _camera.StartAsync();
            CaptureStatusText.Text = $"Using {choice.Name}";
            _autoCaptureGate.Reset();
        }
        catch (UnauthorizedAccessException exception)
        {
            await ShowErrorAsync(
                "Camera permission denied",
                exception,
                "Enable camera access in Windows Settings, then retry.");
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Camera failed", exception);
        }
    }

    private async void SwitchCameraButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (_cameraChoices.Count < 2)
        {
            SetStatus("No second camera is available.");
            return;
        }

        _cameraIndex = (_cameraIndex + 1) % _cameraChoices.Count;
        await StartCameraAsync();
    }

    private async void CaptureButton_Click(
        object sender,
        RoutedEventArgs args) => await CapturePhotoAsync();

    private async Task CapturePhotoAsync()
    {
        if (_camera is null
            || Interlocked.Exchange(ref _capturing, 1) != 0)
        {
            return;
        }

        try
        {
            CaptureStatusText.Text = "Capturing…";
            var bytes = await _camera.CaptureJpegAsync();
            _editingPageIndex = null;
            await StopCameraAsync();
            await BeginReviewAsync(bytes, ".jpg");
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Capture failed", exception);
            CaptureStatusText.Text = "Ready to capture";
        }
        finally
        {
            Interlocked.Exchange(ref _capturing, 0);
        }
    }

    private void Camera_FrameArrived(FramePacket frame)
    {
        if (Interlocked.Exchange(ref _detecting, 1) != 0)
        {
            return;
        }

        _ = Task.Run(() =>
        {
            try
            {
                var detection = DocumentDetector.Detect(frame);
                DispatcherQueue.TryEnqueue(() =>
                {
                    _latestDetection = detection;
                    UpdateDetectionOverlay(frame, detection);
                    if (!AutoCaptureToggle.IsOn)
                    {
                        return;
                    }

                    var state = _autoCaptureGate.Evaluate(
                        detection,
                        frame.Timestamp);
                    CaptureStatusText.Text = state switch
                    {
                        AutoCaptureState.NoDocument => "Find document edges",
                        AutoCaptureState.HoldSteady => "Hold steady…",
                        AutoCaptureState.Ready => "Captured",
                        _ => "Captured",
                    };
                    if (state == AutoCaptureState.Ready)
                    {
                        _ = CapturePhotoAsync();
                    }
                });
            }
            finally
            {
                Interlocked.Exchange(ref _detecting, 0);
            }
        });
    }

    private void UpdateDetectionOverlay(
        FramePacket frame,
        DocumentDetection detection)
    {
        if (detection.Corners is null)
        {
            DetectionPolygon.Points.Clear();
            return;
        }

        var width = DetectionOverlay.ActualWidth;
        var height = DetectionOverlay.ActualHeight;
        var scale = Math.Min(width / frame.Width, height / frame.Height);
        var contentWidth = frame.Width * scale;
        var contentHeight = frame.Height * scale;
        var offsetX = (width - contentWidth) / 2;
        var offsetY = (height - contentHeight) / 2;
        DetectionPolygon.Points.Clear();
        foreach (var corner in detection.Corners.Points)
        {
            var x = frame.Mirrored ? 1 - corner.X : corner.X;
            DetectionPolygon.Points.Add(new Point(
                offsetX + x * contentWidth,
                offsetY + corner.Y * contentHeight));
        }
    }

    private async void CaptureBackButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        await StopCameraAsync();
        if (_project.Pages.Count == 0)
        {
            SetScreen(AppScreen.Home);
        }
        else
        {
            ShowEditor();
        }
    }

    private async Task StopCameraAsync()
    {
        if (_camera is null)
        {
            return;
        }

        var camera = _camera;
        _camera = null;
        camera.FrameArrived -= Camera_FrameArrived;
        camera.Failed -= Camera_Failed;
        await camera.DisposeAsync();
        DetectionPolygon.Points.Clear();
    }

    private async Task BeginReviewAsync(byte[] bytes, string extension)
    {
        _pendingBytes = bytes;
        _pendingExtension = extension.Equals(
            ".png",
            StringComparison.OrdinalIgnoreCase) ? ".png" : ".jpg";
        _reviewSource?.Dispose();
        _reviewSource = SKImage.FromEncodedData(bytes)
            ?? throw new InvalidDataException("Image could not be decoded.");
        ProjectValidator.ValidateImageInput(
            bytes.LongLength,
            _reviewSource.Width,
            _reviewSource.Height);

        if (_editingPageIndex is int editIndex)
        {
            var page = _project.Pages[editIndex];
            _pendingCrop = page.Crop;
            _pendingFilter = page.Filter;
            _pendingRotation = page.RotationQuarterTurns;
        }
        else
        {
            var detection = await Task.Run(
                () => DocumentDetector.DetectEncoded(bytes));
            _pendingCrop = detection.Corners ?? CropQuad.Full;
            _pendingFilter = new PageFilterSettings();
            _pendingRotation = 0;
        }

        FilterPicker.SelectedIndex = (int)_pendingFilter.Mode;
        ContrastSlider.Value = _pendingFilter.Contrast;
        AdjustCornersToggle.IsChecked = true;
        SetScreen(AppScreen.Review);
        await RefreshReviewAsync();
    }

    private async Task RefreshReviewAsync()
    {
        if (_pendingBytes is null || !_pendingCrop.IsConvex())
        {
            return;
        }

        _renderCancellation?.Cancel();
        _renderCancellation?.Dispose();
        _renderCancellation = new CancellationTokenSource();
        var token = _renderCancellation.Token;
        try
        {
            var bytes = _pendingBytes;
            var crop = _pendingCrop;
            var rotation = _pendingRotation;
            var filter = _pendingFilter;
            var processed = await Task.Run(
                () => ImageProcessor.Process(
                    bytes,
                    crop,
                    rotation,
                    filter),
                token);
            token.ThrowIfCancellationRequested();
            var image = SKImage.FromEncodedData(processed.EncodedPng)
                ?? throw new InvalidDataException(
                    "Processed image could not be decoded.");
            _reviewProcessed?.Dispose();
            _reviewProcessed = image;
            ReviewCanvas.Invalidate();
        }
        catch (OperationCanceledException)
        {
            // A newer crop or filter request replaced this render.
        }
        catch (Exception exception)
        {
            SetStatus($"Preview failed: {exception.Message}");
        }
    }

    private void ReviewCanvas_PaintSurface(
        object sender,
        SKPaintSurfaceEventArgs args)
    {
        var canvas = args.Surface.Canvas;
        canvas.Clear(SKColors.Black);
        var adjusting = AdjustCornersToggle.IsChecked == true;
        var image = adjusting ? _reviewSource : _reviewProcessed;
        if (image is null)
        {
            return;
        }

        _reviewImageRect = FitRect(
            args.Info.Width,
            args.Info.Height,
            image.Width,
            image.Height,
            1);
        canvas.DrawImage(
            image,
            _reviewImageRect,
            new SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.Linear));
        if (adjusting)
        {
            DrawCropOverlay(canvas);
        }
    }

    private void DrawCropOverlay(SKCanvas canvas)
    {
        using var builder = new SKPathBuilder();
        var points = _pendingCrop.Points
            .Select(ReviewPoint)
            .ToArray();
        builder.MoveTo(points[0]);
        foreach (var point in points.Skip(1))
        {
            builder.LineTo(point);
        }

        builder.Close();
        using var path = builder.Detach();
        using var line = new SKPaint
        {
            Color = SKColors.DeepSkyBlue,
            IsAntialias = true,
            Style = SKPaintStyle.Stroke,
            StrokeWidth = 4,
        };
        using var handle = new SKPaint
        {
            Color = SKColors.White,
            IsAntialias = true,
            Style = SKPaintStyle.Fill,
        };
        canvas.DrawPath(path, line);
        foreach (var point in points)
        {
            canvas.DrawCircle(point, 12, handle);
            canvas.DrawCircle(point, 12, line);
        }
    }

    private void ReviewCanvas_PointerPressed(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (AdjustCornersToggle.IsChecked != true)
        {
            return;
        }

        var position = ToSkia(args.GetCurrentPoint(ReviewCanvas).Position);
        var distances = _pendingCrop.Points
            .Select((point, index) => new
            {
                Index = index,
                Distance = Distance(ReviewPoint(point), position),
            })
            .OrderBy(value => value.Distance)
            .First();
        if (distances.Distance <= 44)
        {
            _activeCropCorner = distances.Index;
            ReviewCanvas.CapturePointer(args.Pointer);
        }
    }

    private void ReviewCanvas_PointerMoved(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (_activeCropCorner is not int index)
        {
            return;
        }

        var point = ToNormalized(
            ToSkia(args.GetCurrentPoint(ReviewCanvas).Position),
            _reviewImageRect);
        var corners = _pendingCrop.Points.ToArray();
        corners[index] = point.Clamp();
        var candidate = new CropQuad(
            corners[0],
            corners[1],
            corners[2],
            corners[3]);
        if (candidate.IsConvex())
        {
            _pendingCrop = candidate;
            ReviewCanvas.Invalidate();
        }
    }

    private async void ReviewCanvas_PointerReleased(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (_activeCropCorner is null)
        {
            return;
        }

        _activeCropCorner = null;
        ReviewCanvas.ReleasePointerCapture(args.Pointer);
        await RefreshReviewAsync();
    }

    private async void FilterPicker_SelectionChanged(
        object sender,
        SelectionChangedEventArgs args)
    {
        if (_pendingBytes is null || FilterPicker.SelectedIndex < 0)
        {
            return;
        }

        _pendingFilter = _pendingFilter with
        {
            Mode = (PageFilterMode)FilterPicker.SelectedIndex,
        };
        await RefreshReviewAsync();
    }

    private async void ContrastSlider_ValueChanged(
        object sender,
        RangeBaseValueChangedEventArgs args)
    {
        if (_pendingBytes is null)
        {
            return;
        }

        _pendingFilter = _pendingFilter with
        {
            Contrast = (int)Math.Round(args.NewValue),
        };
        await RefreshReviewAsync();
    }

    private void ReviewSetting_Changed(
        object sender,
        RoutedEventArgs args) => ReviewCanvas.Invalidate();

    private async void RotatePageButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        _pendingRotation = (_pendingRotation + 1) % 4;
        await RefreshReviewAsync();
    }

    private async void RetakeButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (_editingPageIndex is not null)
        {
            ClearReview();
            ShowEditor();
            return;
        }

        ClearReview();
        await StartCameraAsync();
    }

    private async void AcceptPageButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (_pendingBytes is null || _reviewSource is null)
        {
            return;
        }

        try
        {
            _history.Checkpoint(_project);
            if (_editingPageIndex is int editIndex)
            {
                var existing = _project.Pages[editIndex];
                _project.Pages[editIndex] = existing with
                {
                    Crop = _pendingCrop,
                    Filter = _pendingFilter,
                    RotationQuarterTurns = _pendingRotation,
                };
                _pageIndex = editIndex;
            }
            else
            {
                var asset = $"{Guid.NewGuid():N}{_pendingExtension}";
                await File.WriteAllBytesAsync(
                    Path.Combine(_workspace, "assets", asset),
                    _pendingBytes);
                _project.Pages.Add(new DocumentPage
                {
                    SourceAsset = asset,
                    SourcePixelWidth = _reviewSource.Width,
                    SourcePixelHeight = _reviewSource.Height,
                    Crop = _pendingCrop,
                    Filter = _pendingFilter,
                    RotationQuarterTurns = _pendingRotation,
                });
                _pageIndex = _project.Pages.Count - 1;
            }

            ClearReview();
            MarkChanged();
            ShowEditor();
            await LoadActivePageAsync();
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Could not add page", exception);
        }
    }

    private void ClearReview()
    {
        _pendingBytes = null;
        _editingPageIndex = null;
        _reviewSource?.Dispose();
        _reviewSource = null;
        _reviewProcessed?.Dispose();
        _reviewProcessed = null;
    }

    private void ShowEditor()
    {
        if (_project.Pages.Count == 0)
        {
            SetScreen(AppScreen.Home);
            return;
        }

        _pageIndex = Math.Clamp(_pageIndex, 0, _project.Pages.Count - 1);
        _updatingPageList = true;
        PageList.Items.Clear();
        for (var index = 0; index < _project.Pages.Count; index++)
        {
            PageList.Items.Add($"Page {index + 1}");
        }

        PageList.SelectedIndex = _pageIndex;
        _updatingPageList = false;
        SetScreen(AppScreen.Editor);
        UpdateCommandState();
    }

    private async Task LoadActivePageAsync()
    {
        if (_project.Pages.Count == 0)
        {
            _processedPage = null;
            _editorBackground?.Dispose();
            _editorBackground = null;
            EditorCanvas.Invalidate();
            return;
        }

        _renderCancellation?.Cancel();
        _renderCancellation?.Dispose();
        _renderCancellation = new CancellationTokenSource();
        var token = _renderCancellation.Token;
        try
        {
            SetStatus($"Rendering page {_pageIndex + 1}…");
            var page = _project.Pages[_pageIndex];
            var processed = await WorkspaceStore.ProcessPageAsync(
                _workspace,
                page,
                token);
            token.ThrowIfCancellationRequested();
            var image = SKImage.FromEncodedData(processed.EncodedPng)
                ?? throw new InvalidDataException("Page image is invalid.");
            _processedPage = processed;
            _editorBackground?.Dispose();
            _editorBackground = image;
            _selectedAnnotationId = null;
            EditorCanvas.Invalidate();
            SetStatus($"Page {_pageIndex + 1} of {_project.Pages.Count}");
        }
        catch (OperationCanceledException)
        {
            // A newer page request replaced this render.
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Page render failed", exception);
        }
    }

    private void EditorCanvas_PaintSurface(
        object sender,
        SKPaintSurfaceEventArgs args)
    {
        var canvas = args.Surface.Canvas;
        canvas.Clear(new SKColor(0xff202020));
        if (_editorBackground is null || _project.Pages.Count == 0)
        {
            return;
        }

        _editorImageRect = FitRect(
            args.Info.Width,
            args.Info.Height,
            _editorBackground.Width,
            _editorBackground.Height,
            _zoom);
        var widthPoints = _processedPage is null
            ? 612
            : _processedPage.Width / PdfExporter.DotsPerInch * 72;
        PageRenderer.Draw(
            canvas,
            _editorBackground,
            _project.Pages[_pageIndex],
            _editorImageRect,
            new RenderOptions(
                widthPoints,
                _selectedAnnotationId,
                DrawSelection: true));
    }

    private void EditorCanvas_PointerPressed(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (!TryGetPagePoint(args, out var point))
        {
            return;
        }

        _pointerStart = point;
        if (_tool == ToolMode.Select)
        {
            var hit = AnnotationHitTester.HitTest(
                CurrentPage.Annotations,
                point,
                16 / Math.Max(_editorImageRect.Width, 1));
            _selectedAnnotationId = hit?.Id;
            _dragStartTransform = hit?.Transform;
            if (hit is not null)
            {
                _history.Checkpoint(_project);
            }

            EditorCanvas.Invalidate();
            EditorCanvas.CapturePointer(args.Pointer);
            return;
        }

        if (_tool == ToolMode.Text)
        {
            EditorCanvas.CapturePointer(args.Pointer);
            return;
        }

        _history.Checkpoint(_project);
        Annotation annotation = _tool switch
        {
            ToolMode.Pen or ToolMode.Highlighter =>
                new FreehandAnnotation
                {
                    ColorArgb = _toolColor,
                    StrokeWidth = (float)ThicknessSlider.Value,
                    IsHighlighter = _tool == ToolMode.Highlighter,
                    Points = [point],
                },
            _ => new ShapeAnnotation
            {
                ColorArgb = _toolColor,
                StrokeWidth = (float)ThicknessSlider.Value,
                Shape = ToShapeKind(_tool),
                Start = point,
                End = point,
                Filled = FilledToggle.IsOn,
            },
        };
        CurrentPage.Annotations.Add(annotation);
        _draftAnnotationId = annotation.Id;
        _selectedAnnotationId = annotation.Id;
        EditorCanvas.CapturePointer(args.Pointer);
        EditorCanvas.Invalidate();
    }

    private void EditorCanvas_PointerMoved(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (_pointerStart is null
            || !TryGetPagePoint(args, out var point))
        {
            return;
        }

        if (_tool == ToolMode.Select
            && _selectedAnnotationId is Guid selected
            && _dragStartTransform is AffineTransform start)
        {
            var offset = point.Vector - _pointerStart.Value.Vector;
            var matrix = start.Matrix
                * Matrix3x2.CreateTranslation(offset);
            ReplaceAnnotation(
                selected,
                annotation => WithTransform(annotation, matrix));
        }
        else if (_draftAnnotationId is Guid draft)
        {
            ReplaceAnnotation(draft, annotation => annotation switch
            {
                FreehandAnnotation ink => ink with
                {
                    Points = AppendPoint(ink.Points, point),
                },
                ShapeAnnotation shape => shape with { End = point },
                _ => annotation,
            });
        }

        EditorCanvas.Invalidate();
    }

    private async void EditorCanvas_PointerReleased(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (_pointerStart is null)
        {
            return;
        }

        EditorCanvas.ReleasePointerCapture(args.Pointer);
        var changed = _draftAnnotationId is not null
            || _tool == ToolMode.Select
            && _selectedAnnotationId is not null
            && _dragStartTransform is not null;
        if (_tool == ToolMode.Text
            && TryGetPagePoint(args, out var end))
        {
            changed = await AddTextAsync(_pointerStart.Value, end);
        }

        _pointerStart = null;
        _draftAnnotationId = null;
        _dragStartTransform = null;
        if (changed)
        {
            MarkChanged();
        }

        EditorCanvas.Invalidate();
    }

    private async Task<bool> AddTextAsync(
        NormalizedPoint start,
        NormalizedPoint end)
    {
        var input = new TextBox
        {
            AcceptsReturn = true,
            Header = "Text",
            MinWidth = 360,
            MinHeight = 120,
        };
        var dialog = new ContentDialog
        {
            Title = "Add text",
            Content = input,
            PrimaryButtonText = "Add",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary
            || string.IsNullOrWhiteSpace(input.Text))
        {
            return false;
        }

        _history.Checkpoint(_project);
        var bounds = NormalizedRect.FromPoints(start, end);
        if (bounds.Width < 0.08f || bounds.Height < 0.04f)
        {
            bounds = new NormalizedRect(
                start.X,
                start.Y,
                Math.Min(0.4f, 1 - start.X),
                Math.Min(0.12f, 1 - start.Y));
        }

        var annotation = new TextAnnotation
        {
            ColorArgb = _toolColor,
            StrokeWidth = (float)ThicknessSlider.Value,
            Text = input.Text.Trim(),
            Bounds = bounds,
            FontSize = Math.Max(10, (float)ThicknessSlider.Value * 5),
        };
        CurrentPage.Annotations.Add(annotation);
        _selectedAnnotationId = annotation.Id;
        return true;
    }

    private void ToolButton_Click(object sender, RoutedEventArgs args)
    {
        if (sender is not ToggleButton selected
            || selected.Tag is not string name
            || !Enum.TryParse(name, out ToolMode tool))
        {
            return;
        }

        foreach (var button in ToolPanel.Children.OfType<ToggleButton>())
        {
            button.IsChecked = ReferenceEquals(button, selected);
        }

        _tool = tool;
        if (tool != ToolMode.Select)
        {
            _selectedAnnotationId = null;
            EditorCanvas.Invalidate();
        }
    }

    private void ColorButton_Click(object sender, RoutedEventArgs args)
    {
        if (sender is not Button { Tag: string value }
            || !uint.TryParse(
                value,
                NumberStyles.HexNumber,
                CultureInfo.InvariantCulture,
                out var color))
        {
            return;
        }

        _toolColor = color;
        ChangeSelected(annotation => annotation with
        {
            ColorArgb = color,
        });
    }

    private void ThicknessSlider_ValueChanged(
        object sender,
        RangeBaseValueChangedEventArgs args) =>
        ChangeSelected(annotation => annotation with
        {
            StrokeWidth = (float)args.NewValue,
        });

    private void FilledToggle_Toggled(
        object sender,
        RoutedEventArgs args) =>
        ChangeSelected(annotation => annotation is ShapeAnnotation shape
            ? shape with { Filled = FilledToggle.IsOn }
            : annotation);

    private void RotateObjectButton_Click(
        object sender,
        RoutedEventArgs args) => TransformSelected(
            Matrix3x2.CreateRotation(MathF.PI / 12));

    private void SmallerObjectButton_Click(
        object sender,
        RoutedEventArgs args) => TransformSelected(
            Matrix3x2.CreateScale(0.9f));

    private void LargerObjectButton_Click(
        object sender,
        RoutedEventArgs args) => TransformSelected(
            Matrix3x2.CreateScale(1.1f));

    private void TransformSelected(Matrix3x2 operation)
    {
        if (SelectedAnnotation is not Annotation selected)
        {
            return;
        }

        _history.Checkpoint(_project);
        var center = AnnotationCenter(selected);
        var centered = Matrix3x2.CreateTranslation(-center)
            * operation
            * Matrix3x2.CreateTranslation(center);
        ReplaceAnnotation(
            selected.Id,
            annotation => WithTransform(
                annotation,
                annotation.Transform.Matrix * centered));
        MarkChanged();
        EditorCanvas.Invalidate();
    }

    private void DeleteObjectButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (_selectedAnnotationId is not Guid id)
        {
            return;
        }

        _history.Checkpoint(_project);
        CurrentPage.Annotations.RemoveAll(annotation => annotation.Id == id);
        _selectedAnnotationId = null;
        MarkChanged();
        EditorCanvas.Invalidate();
    }

    private void ZoomOutButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        _zoom = Math.Max(0.5f, _zoom / 1.25f);
        EditorCanvas.Invalidate();
    }

    private void FitButton_Click(object sender, RoutedEventArgs args)
    {
        _zoom = 1;
        EditorCanvas.Invalidate();
    }

    private void ZoomInButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        _zoom = Math.Min(4, _zoom * 1.25f);
        EditorCanvas.Invalidate();
    }

    private async void PageList_SelectionChanged(
        object sender,
        SelectionChangedEventArgs args)
    {
        if (_updatingPageList
            || PageList.SelectedIndex < 0
            || PageList.SelectedIndex == _pageIndex)
        {
            return;
        }

        _pageIndex = PageList.SelectedIndex;
        await LoadActivePageAsync();
    }

    private async void EditPageButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (_project.Pages.Count == 0)
        {
            return;
        }

        try
        {
            var page = CurrentPage;
            var bytes = await File.ReadAllBytesAsync(
                Path.Combine(_workspace, "assets", page.SourceAsset));
            _editingPageIndex = _pageIndex;
            await BeginReviewAsync(bytes, Path.GetExtension(page.SourceAsset));
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Could not edit page", exception);
        }
    }

    private async void DeletePageButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (_project.Pages.Count == 0)
        {
            return;
        }

        var dialog = new ContentDialog
        {
            Title = "Delete page?",
            Content = $"Page {_pageIndex + 1} will be removed.",
            PrimaryButtonText = "Delete",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }

        _history.Checkpoint(_project);
        _project.Pages.RemoveAt(_pageIndex);
        _pageIndex = Math.Min(_pageIndex, _project.Pages.Count - 1);
        MarkChanged();
        ShowEditor();
        await LoadActivePageAsync();
    }

    private async void MovePageEarlierButton_Click(
        object sender,
        RoutedEventArgs args) => await MovePageAsync(-1);

    private async void MovePageLaterButton_Click(
        object sender,
        RoutedEventArgs args) => await MovePageAsync(1);

    private async Task MovePageAsync(int offset)
    {
        var target = _pageIndex + offset;
        if (target < 0 || target >= _project.Pages.Count)
        {
            return;
        }

        _history.Checkpoint(_project);
        (_project.Pages[_pageIndex], _project.Pages[target]) =
            (_project.Pages[target], _project.Pages[_pageIndex]);
        _pageIndex = target;
        MarkChanged();
        ShowEditor();
        await LoadActivePageAsync();
    }

    private async void UndoButton_Click(
        object sender,
        RoutedEventArgs args) => await UndoAsync();

    private async Task UndoAsync()
    {
        if (!_history.TryUndo(_project, out var project))
        {
            return;
        }

        _project = project;
        _pageIndex = Math.Clamp(_pageIndex, 0, _project.Pages.Count - 1);
        MarkChanged();
        ShowEditor();
        await LoadActivePageAsync();
    }

    private async void RedoButton_Click(
        object sender,
        RoutedEventArgs args) => await RedoAsync();

    private async Task RedoAsync()
    {
        if (!_history.TryRedo(_project, out var project))
        {
            return;
        }

        _project = project;
        _pageIndex = Math.Clamp(_pageIndex, 0, _project.Pages.Count - 1);
        MarkChanged();
        ShowEditor();
        await LoadActivePageAsync();
    }

    private async void SaveButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        try
        {
            if (_projectPath is null)
            {
                var picker = new FileSavePicker
                {
                    SuggestedFileName = SafeSuggestedName(_project.Title),
                };
                InitializePicker(picker);
                picker.FileTypeChoices.Add(
                    "FastFill project",
                    [".docscan"]);
                var file = await picker.PickSaveFileAsync();
                if (file is null)
                {
                    return;
                }

                _projectPath = file.Path;
            }

            await ProjectArchive.SaveAsync(
                _project,
                _workspace,
                _projectPath);
            _dirty = false;
            SetStatus("Project saved.");
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Save failed", exception);
        }
    }

    private async void ExportButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        try
        {
            var picker = new FileSavePicker
            {
                SuggestedFileName = SafeSuggestedName(_project.Title),
            };
            InitializePicker(picker);
            picker.FileTypeChoices.Add("PDF document", [".pdf"]);
            var file = await picker.PickSaveFileAsync();
            if (file is null)
            {
                return;
            }

            await ExportPdfAsync(file.Path);
            SetStatus($"Exported {_project.Pages.Count} page PDF.");
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("PDF export failed", exception);
        }
    }

    private async Task ExportPdfAsync(string destination)
    {
        var temporary = destination + $".{Guid.NewGuid():N}.tmp";
        try
        {
            await using var stream = new FileStream(
                temporary,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                64 * 1024,
                FileOptions.Asynchronous | FileOptions.WriteThrough);
            await PdfExporter.ExportAsync(
                _project,
                (page, token) => WorkspaceStore.ProcessPageAsync(
                    _workspace,
                    page,
                    token),
                stream);
            stream.Close();
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

    private async void ShareButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        try
        {
            var path = Path.Combine(
                ApplicationData.Current.TemporaryFolder.Path,
                "FastFill-share.pdf");
            await ExportPdfAsync(path);
            _shareFile = await StorageFile.GetFileFromPathAsync(path);
            var interop = DataTransferManager
                .As<IDataTransferManagerInterop>();
            interop.ShowShareUIForWindow(WindowHandle);
        }
        catch (Exception exception)
        {
            await ShowErrorAsync("Share failed", exception);
        }
    }

    private void InitializeShare()
    {
        var interop = DataTransferManager.As<IDataTransferManagerInterop>();
        var id = DataTransferManagerId;
        var pointer = interop.GetForWindow(WindowHandle, ref id);
        _dataTransferManager = MarshalInterface<DataTransferManager>
            .FromAbi(pointer);
        _dataTransferManager.DataRequested += DataTransfer_DataRequested;
    }

    private void DataTransfer_DataRequested(
        DataTransferManager sender,
        DataRequestedEventArgs args)
    {
        args.Request.Data.Properties.Title = _project.Title;
        args.Request.Data.Properties.Description =
            "PDF created with FastFill";
        if (_shareFile is null)
        {
            args.Request.FailWithDisplayText("PDF is not ready.");
            return;
        }

        args.Request.Data.SetStorageItems([_shareFile]);
    }

    private void ChangeSelected(Func<Annotation, Annotation> update)
    {
        if (_selectedAnnotationId is not Guid id)
        {
            return;
        }

        _history.Checkpoint(_project);
        ReplaceAnnotation(id, update);
        MarkChanged();
        EditorCanvas.Invalidate();
    }

    private void ReplaceAnnotation(
        Guid id,
        Func<Annotation, Annotation> update)
    {
        var index = CurrentPage.Annotations.FindIndex(
            annotation => annotation.Id == id);
        if (index >= 0)
        {
            CurrentPage.Annotations[index] = update(
                CurrentPage.Annotations[index]);
        }
    }

    private void MarkChanged()
    {
        _project = _project with { UpdatedUtc = DateTimeOffset.UtcNow };
        _dirty = true;
        UpdateCommandState();
        _autosaveCancellation?.Cancel();
        _autosaveCancellation?.Dispose();
        _autosaveCancellation = new CancellationTokenSource();
        _ = AutosaveAsync(_autosaveCancellation.Token);
    }

    private async Task AutosaveAsync(CancellationToken token)
    {
        try
        {
            await Task.Delay(400, token);
            await WorkspaceStore.SaveManifestAsync(
                _project,
                _workspace,
                token);
        }
        catch (OperationCanceledException)
        {
            // A newer edit replaced this autosave.
        }
        catch (Exception exception)
        {
            DispatcherQueue.TryEnqueue(() =>
                SetStatus($"Autosave failed: {exception.Message}"));
        }
    }

    private void UpdateCommandState()
    {
        var hasPages = _project.Pages.Count > 0;
        UndoButton.IsEnabled = _history.CanUndo;
        RedoButton.IsEnabled = _history.CanRedo;
        SaveButton.IsEnabled = hasPages;
        ExportButton.IsEnabled = hasPages;
        ShareButton.IsEnabled = hasPages;
    }

    private void SetScreen(AppScreen screen)
    {
        _screen = screen;
        HomePanel.Visibility = screen == AppScreen.Home
            ? Visibility.Visible
            : Visibility.Collapsed;
        CapturePanel.Visibility = screen == AppScreen.Capture
            ? Visibility.Visible
            : Visibility.Collapsed;
        ReviewPanel.Visibility = screen == AppScreen.Review
            ? Visibility.Visible
            : Visibility.Collapsed;
        EditorPanel.Visibility = screen == AppScreen.Editor
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private void InstallKeyboardShortcuts()
    {
        AddShortcut(VirtualKey.Z, VirtualKeyModifiers.Control, UndoAsync);
        AddShortcut(VirtualKey.Y, VirtualKeyModifiers.Control, RedoAsync);
    }

    private void AddShortcut(
        VirtualKey key,
        VirtualKeyModifiers modifiers,
        Func<Task> action)
    {
        var shortcut = new KeyboardAccelerator
        {
            Key = key,
            Modifiers = modifiers,
        };
        shortcut.Invoked += (sender, args) =>
        {
            args.Handled = true;
            _ = action();
        };
        ((UIElement)Content).KeyboardAccelerators.Add(shortcut);
    }

    private async void MainWindow_VisibilityChanged(
        object sender,
        WindowVisibilityChangedEventArgs args)
    {
        if (!args.Visible && _camera is not null)
        {
            await StopCameraAsync();
        }
        else if (args.Visible
            && _screen == AppScreen.Capture
            && _camera is null)
        {
            await StartCameraAsync();
        }
    }

    private void MainWindow_Closed(object sender, WindowEventArgs args)
    {
        _renderCancellation?.Cancel();
        _autosaveCancellation?.Cancel();
        if (_dirty && !string.IsNullOrEmpty(_workspace))
        {
            try
            {
                Task.Run(() => WorkspaceStore.SaveManifestAsync(
                        _project,
                        _workspace))
                    .GetAwaiter()
                    .GetResult();
            }
            catch
            {
                // Keep shutdown reliable; prior autosave remains recoverable.
            }
        }

        _reviewSource?.Dispose();
        _reviewProcessed?.Dispose();
        _editorBackground?.Dispose();
        _renderCancellation?.Dispose();
        _autosaveCancellation?.Dispose();
        if (_dataTransferManager is not null)
        {
            _dataTransferManager.DataRequested -= DataTransfer_DataRequested;
        }

        _ = StopCameraAsync();
    }

    private void Camera_Failed(string message) =>
        DispatcherQueue.TryEnqueue(() =>
            CaptureStatusText.Text = $"Camera stopped: {message}");

    private void ShowRecoverIfAvailable()
    {
        RecoverButton.Visibility = FindLatestRecoveryManifest() is null
            ? Visibility.Collapsed
            : Visibility.Visible;
    }

    private string? FindLatestRecoveryManifest()
    {
        if (!Directory.Exists(_workspaceRoot))
        {
            return null;
        }

        return Directory.EnumerateFiles(
                _workspaceRoot,
                "project.json",
                SearchOption.AllDirectories)
            .OrderByDescending(File.GetLastWriteTimeUtc)
            .FirstOrDefault();
    }

    private async Task ShowErrorAsync(
        string title,
        Exception exception,
        string? advice = null)
    {
        SetStatus($"{title}: {exception.Message}");
        var message = advice is null
            ? exception.Message
            : $"{advice}\n\n{exception.Message}";
        var dialog = new ContentDialog
        {
            Title = title,
            Content = message,
            CloseButtonText = "OK",
            XamlRoot = XamlRoot,
        };
        await dialog.ShowAsync();
    }

    private void SetStatus(string text) => StatusText.Text = text;

    private void InitializePicker(object picker) =>
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WindowHandle);

    private static async Task<byte[]> ReadStorageFileAsync(StorageFile file)
    {
        var buffer = await FileIO.ReadBufferAsync(file);
        if (buffer.Length > ProjectValidator.MaximumImageBytes)
        {
            throw new InvalidDataException("Image exceeds 100 MiB.");
        }

        using var reader = DataReader.FromBuffer(buffer);
        var bytes = new byte[checked((int)buffer.Length)];
        reader.ReadBytes(bytes);
        return bytes;
    }

    private bool TryGetPagePoint(
        PointerRoutedEventArgs args,
        out NormalizedPoint point)
    {
        var position = ToSkia(
            args.GetCurrentPoint(EditorCanvas).Position);
        if (!_editorImageRect.Contains(position))
        {
            point = default;
            return false;
        }

        point = ToNormalized(position, _editorImageRect);
        return true;
    }

    private SKPoint ReviewPoint(NormalizedPoint point) => new(
        _reviewImageRect.Left + point.X * _reviewImageRect.Width,
        _reviewImageRect.Top + point.Y * _reviewImageRect.Height);

    private static NormalizedPoint ToNormalized(
        SKPoint point,
        SKRect rectangle) => new(
        (point.X - rectangle.Left) / rectangle.Width,
        (point.Y - rectangle.Top) / rectangle.Height);

    private static SKPoint ToSkia(Point point) =>
        new((float)point.X, (float)point.Y);

    private static float Distance(SKPoint first, SKPoint second) =>
        SKPoint.Distance(first, second);

    private static SKRect FitRect(
        float availableWidth,
        float availableHeight,
        float contentWidth,
        float contentHeight,
        float zoom)
    {
        var scale = Math.Min(
            availableWidth / contentWidth,
            availableHeight / contentHeight) * zoom;
        var width = contentWidth * scale;
        var height = contentHeight * scale;
        return new SKRect(
            (availableWidth - width) / 2,
            (availableHeight - height) / 2,
            (availableWidth + width) / 2,
            (availableHeight + height) / 2);
    }

    private static ShapeKind ToShapeKind(ToolMode tool) => tool switch
    {
        ToolMode.Line => ShapeKind.Line,
        ToolMode.Arrow => ShapeKind.Arrow,
        ToolMode.Rectangle => ShapeKind.Rectangle,
        ToolMode.Ellipse => ShapeKind.Ellipse,
        ToolMode.Checkmark => ShapeKind.Checkmark,
        _ => throw new ArgumentOutOfRangeException(nameof(tool)),
    };

    private static Annotation WithTransform(
        Annotation annotation,
        Matrix3x2 matrix) => annotation switch
    {
        FreehandAnnotation value => value with
        {
            Transform = AffineTransform.FromMatrix(matrix),
        },
        TextAnnotation value => value with
        {
            Transform = AffineTransform.FromMatrix(matrix),
        },
        ShapeAnnotation value => value with
        {
            Transform = AffineTransform.FromMatrix(matrix),
        },
        _ => annotation,
    };

    private static List<NormalizedPoint> AppendPoint(
        List<NormalizedPoint> points,
        NormalizedPoint point)
    {
        points.Add(point);
        return points;
    }

    private static Vector2 AnnotationCenter(Annotation annotation)
    {
        var bounds = annotation switch
        {
            FreehandAnnotation ink when ink.Points.Count > 0 => new
                NormalizedRect(
                    ink.Points.Min(point => point.X),
                    ink.Points.Min(point => point.Y),
                    ink.Points.Max(point => point.X)
                        - ink.Points.Min(point => point.X),
                    ink.Points.Max(point => point.Y)
                        - ink.Points.Min(point => point.Y)),
            TextAnnotation text => text.Bounds,
            ShapeAnnotation shape => NormalizedRect.FromPoints(
                shape.Start,
                shape.End),
            _ => new NormalizedRect(0, 0, 0, 0),
        };
        var localCenter = new Vector2(
            bounds.X + bounds.Width / 2,
            bounds.Y + bounds.Height / 2);
        return Vector2.Transform(localCenter, annotation.Transform.Matrix);
    }

    private static string SafeSuggestedName(string title)
    {
        var invalid = Path.GetInvalidFileNameChars();
        var safe = new string(title
            .Select(character => invalid.Contains(character) ? '_' : character)
            .ToArray())
            .Trim();
        return string.IsNullOrWhiteSpace(safe) ? "FastFill scan" : safe;
    }

    private DocumentPage CurrentPage => _project.Pages[_pageIndex];

    private Annotation? SelectedAnnotation =>
        _selectedAnnotationId is Guid id
            ? CurrentPage.Annotations.FirstOrDefault(
                annotation => annotation.Id == id)
            : null;

    private IntPtr WindowHandle =>
        WinRT.Interop.WindowNative.GetWindowHandle(this);

    private FrameworkElement XamlRootElement =>
        (FrameworkElement)Content;

    private XamlRoot XamlRoot => XamlRootElement.XamlRoot;

    private enum AppScreen
    {
        Home,
        Capture,
        Review,
        Editor,
    }

    private enum ToolMode
    {
        Select,
        Text,
        Pen,
        Highlighter,
        Line,
        Arrow,
        Rectangle,
        Ellipse,
        Checkmark,
    }

    [ComImport]
    [Guid("3A3DCD6C-3EAB-43DC-BCDE-45671CE800C8")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IDataTransferManagerInterop
    {
        IntPtr GetForWindow(IntPtr appWindow, ref Guid interfaceId);

        void ShowShareUIForWindow(IntPtr appWindow);
    }
}
