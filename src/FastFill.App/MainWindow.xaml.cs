using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.Numerics;
using System.Runtime.InteropServices;
using System.Text.Json;
using FastFill.Core;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using SkiaSharp;
using SkiaSharp.Views.Windows;
using Windows.ApplicationModel.DataTransfer;
using Windows.Data.Pdf;
using Windows.Foundation;
using Windows.Storage;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;
using Windows.System;
using Windows.UI.Core;
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
    private readonly Dictionary<ToolMode, ToolSettings> _toolSettings =
        CreateToolSettings();
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
    private NormalizedPoint? _pointerStart;
    private readonly HashSet<Guid> _selectedAnnotationIds = [];
    private Dictionary<Guid, AffineTransform>? _dragStartTransforms;
    private Annotation? _resizeStartAnnotation;
    private int _resizeCornerIndex;
    private bool _resizingSelection;
    private AffineTransform? _rotationStartTransform;
    private Vector2 _rotationCenter;
    private float _rotationStartAngle;
    private bool _rotatingSelection;
    private NormalizedPoint? _selectionRectangleStart;
    private NormalizedPoint? _selectionRectangleEnd;
    private bool _addRectangleSelection;
    private Guid? _toggleSelectionOnClick;
    private bool _pointerMoved;
    private FastFillProject? _interactionSnapshot;
    private Guid? _draftAnnotationId;
    private Guid? _textEditAnnotationId;
    private NormalizedRect _textEditBounds;
    private AffineTransform _textEditTransform = AffineTransform.Identity;
    private bool _closingTextEditor;
    private float _zoom = 1;
    private Vector2 _pan;
    private Vector2 _editorViewportSize;
    private readonly Dictionary<uint, Point> _navigationPoints = [];
    private double _gestureStartDistance;
    private Point _gestureStartCenter;
    private float _gestureStartZoom;
    private Vector2 _gestureStartPan;
    private bool _dirty;
    private bool _updatingPageList;
    private bool _reorderingPages;
    private Guid? _activePageBeforeReorder;
    private bool _updatingToolOptions;
    private bool _controlsReady;
    private DataTransferManager? _dataTransferManager;
    private StorageFile? _shareFile;
    private AppScreen _screen = AppScreen.Home;

    public MainWindow()
    {
        InitializeComponent();
        AppWindow.SetIcon(Path.Combine(
            AppContext.BaseDirectory,
            "Assets",
            "AppIcon.ico"));
        _workspaceRoot = Path.Combine(
            Environment.GetFolderPath(
                Environment.SpecialFolder.LocalApplicationData),
            "FastFill",
            "workspaces");
        Directory.CreateDirectory(_workspaceRoot);
        InitializeShare();
        _controlsReady = true;
        ApplyToolOptions();
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
            picker.FileTypeFilter.Add(".pdf");
            var file = await picker.PickSingleFileAsync();
            if (file is null)
            {
                return;
            }

            if (file.FileType.Equals(
                ".pdf",
                StringComparison.OrdinalIgnoreCase))
            {
                await ImportPdfAsync(file);
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

    private async Task ImportPdfAsync(StorageFile file)
    {
        var properties = await file.GetBasicPropertiesAsync();
        if (properties.Size == 0
            || properties.Size
                > checked((ulong)ProjectValidator.MaximumImageBytes))
        {
            throw new InvalidDataException(
                "PDF must be between 1 byte and 100 MiB.");
        }

        var document = await PdfDocument.LoadFromFileAsync(file);
        if (document.PageCount == 0)
        {
            throw new InvalidDataException("PDF contains no pages.");
        }

        var available = FastFillProject.MaximumPages - _project.Pages.Count;
        if (document.PageCount > checked((uint)available))
        {
            throw new InvalidDataException(
                $"PDF has {document.PageCount} pages; "
                + $"this project has room for {available}.");
        }

        SetStatus($"Importing {document.PageCount} PDF pages…");
        var renderedPages = new List<PdfImportPage>();
        for (uint index = 0; index < document.PageCount; index++)
        {
            using var page = document.GetPage(index);
            var width = checked((int)Math.Ceiling(
                page.Size.Width
                * PdfExporter.DotsPerInch
                / 96));
            var height = checked((int)Math.Ceiling(
                page.Size.Height
                * PdfExporter.DotsPerInch
                / 96));
            ProjectValidator.ValidateImageInput(1, width, height);
            using var stream = new InMemoryRandomAccessStream();
            var options = new PdfPageRenderOptions
            {
                DestinationWidth = checked((uint)width),
                DestinationHeight = checked((uint)height),
                BitmapEncoderId = Windows.Graphics.Imaging.BitmapEncoder
                    .PngEncoderId,
            };
            await page.RenderToStreamAsync(stream, options);
            if (stream.Size
                > checked((ulong)ProjectValidator.MaximumImageBytes))
            {
                throw new InvalidDataException(
                    $"Rendered PDF page {index + 1} exceeds 100 MiB.");
            }

            var length = checked((uint)stream.Size);
            using var reader = new DataReader(stream.GetInputStreamAt(0));
            var loaded = await reader.LoadAsync(length);
            if (loaded != length)
            {
                throw new InvalidDataException(
                    $"Could not read rendered PDF page {index + 1}.");
            }

            var bytes = new byte[checked((int)length)];
            reader.ReadBytes(bytes);
            ProjectValidator.ValidateImageInput(
                bytes.LongLength,
                width,
                height);
            renderedPages.Add(new PdfImportPage(bytes, width, height));
            SetStatus(
                $"Importing PDF page {index + 1} of "
                + $"{document.PageCount}…");
        }

        var firstPage = _project.Pages.Count;
        var importedPages = new List<DocumentPage>();
        foreach (var rendered in renderedPages)
        {
            var asset = $"{Guid.NewGuid():N}.png";
            await File.WriteAllBytesAsync(
                Path.Combine(_workspace, "assets", asset),
                rendered.Bytes);
            importedPages.Add(new DocumentPage
            {
                SourceAsset = asset,
                SourcePixelWidth = rendered.Width,
                SourcePixelHeight = rendered.Height,
            });
        }

        _history.Checkpoint(_project);
        _project.Pages.AddRange(importedPages);

        if (firstPage == 0)
        {
            _project = _project with
            {
                Title = Path.GetFileNameWithoutExtension(file.Name),
            };
        }

        _pageIndex = firstPage;
        MarkChanged();
        ShowEditor();
        await LoadActivePageAsync();
        SetStatus($"Imported {renderedPages.Count} PDF pages.");
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
                RecoverButton.IsEnabled = false;
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
            CaptureCountdownRing.Value = 0;
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
            _autoCaptureGate.Reset();
            CaptureCountdownRing.Value = 0;
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
                        CaptureCountdownRing.Value = 0;
                        return;
                    }

                    var state = _autoCaptureGate.Evaluate(
                        detection,
                        frame.Timestamp);
                    CaptureCountdownRing.Value =
                        _autoCaptureGate.Progress * 100;
                    CaptureStatusText.Text = state switch
                    {
                        AutoCaptureState.NoDocument => "Find document edges",
                        AutoCaptureState.HoldSteady =>
                            $"Hold steady "
                            + $"{RemainingCountdownSeconds():0.0}s",
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

    private double RemainingCountdownSeconds() => Math.Max(
        0,
        AutoCaptureGate.CountdownDuration.TotalSeconds
            * (1 - _autoCaptureGate.Progress));

    private void AutoCaptureToggle_Toggled(
        object sender,
        RoutedEventArgs args)
    {
        if (!_controlsReady)
        {
            return;
        }

        _autoCaptureGate.Reset();
        CaptureCountdownRing.Value = 0;
        CaptureStatusText.Text = AutoCaptureToggle.IsOn
            ? "Find document edges"
            : "Auto capture off";
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
        AdjustCornersToggle.IsChecked = false;
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
        AdjustCornersToggle.IsChecked = false;
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
        AdjustCornersToggle.IsChecked = false;
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
        var pageIds = _project.Pages.Select(page => page.Id).ToArray();
        var displayedIds = PageList.Items
            .OfType<PageListItem>()
            .Select(item => item.PageId);
        if (!displayedIds.SequenceEqual(pageIds))
        {
            _updatingPageList = true;
            PageList.Items.Clear();
            for (var index = 0; index < _project.Pages.Count; index++)
            {
                PageList.Items.Add(new PageListItem(
                    _project.Pages[index].Id,
                    $"Page {index + 1}"));
            }

            _updatingPageList = false;
        }

        if (PageList.SelectedIndex != _pageIndex)
        {
            _updatingPageList = true;
            PageList.SelectedIndex = _pageIndex;
            _updatingPageList = false;
        }

        SetScreen(AppScreen.Editor);
        UpdateCommandState();
    }

    private async Task LoadActivePageAsync()
    {
        CancelInlineTextEdit();
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
            _selectedAnnotationIds.Clear();
            ApplyToolOptions();
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

        _editorViewportSize = new Vector2(
            args.Info.Width,
            args.Info.Height);
        ClampPan();
        var imageRect = FitRect(
            args.Info.Width,
            args.Info.Height,
            _editorBackground.Width,
            _editorBackground.Height,
            _zoom);
        _editorImageRect = new SKRect(
            imageRect.Left + _pan.X,
            imageRect.Top + _pan.Y,
            imageRect.Right + _pan.X,
            imageRect.Bottom + _pan.Y);
        var widthPoints = _processedPage is null
            ? 612
            : _processedPage.Width / PdfExporter.DotsPerInch * 72;
        var heightPoints = _processedPage is null
            ? 792
            : _processedPage.Height / PdfExporter.DotsPerInch * 72;
        PageRenderer.Draw(
            canvas,
            _editorBackground,
            _project.Pages[_pageIndex],
            _editorImageRect,
            new RenderOptions(
                widthPoints,
                heightPoints,
                _selectedAnnotationIds,
                DrawSelection: true,
                HiddenAnnotationId: _textEditAnnotationId,
                TransformAnnotationId: SelectedAnnotation?.Id,
                SelectionRectangle: SelectionRectangle));
        PositionInlineTextEditor(widthPoints);
    }

    private void EditorCanvas_PointerPressed(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (HandleNavigationPressed(args))
        {
            return;
        }

        if (!TryGetPagePoint(
            args,
            out var point,
            allowSelectionMargin: _tool == ToolMode.Select))
        {
            return;
        }

        _pointerStart = point;
        _pointerMoved = false;
        _interactionSnapshot = null;
        ResetSelectionInteraction();
        if (_tool == ToolMode.Select)
        {
            var control = IsKeyDown(VirtualKey.Control);
            var shift = IsKeyDown(VirtualKey.Shift);
            if (!shift
                && !control
                && TryBeginSelectionRotation(point))
            {
                EditorCanvas.CapturePointer(args.Pointer);
                return;
            }

            var forceResize = control
                && SelectedAnnotation is Annotation selected
                && AnnotationHitTester.BoundsContain(selected, point);
            if (!shift
                && TryBeginSelectionResize(point, forceResize))
            {
                _toggleSelectionOnClick = control
                    ? SelectedAnnotation?.Id
                    : null;
                EditorCanvas.CapturePointer(args.Pointer);
                return;
            }

            var selectedHit = CurrentPage.Annotations
                .LastOrDefault(annotation =>
                    _selectedAnnotationIds.Contains(annotation.Id)
                    && AnnotationHitTester.BoundsContain(annotation, point));
            if (selectedHit is not null)
            {
                if (control)
                {
                    _selectedAnnotationIds.Remove(selectedHit.Id);
                }
                else
                {
                    BeginSelectionMove();
                }

                ApplyToolOptions();
                EditorCanvas.Invalidate();
                EditorCanvas.CapturePointer(args.Pointer);
                return;
            }

            if (point.X is < 0 or > 1 || point.Y is < 0 or > 1)
            {
                _pointerStart = null;
                return;
            }

            var hit = AnnotationHitTester.HitTest(
                CurrentPage.Annotations,
                point,
                16 / Math.Max(_editorImageRect.Width, 1));
            if (hit is null)
            {
                if (!control)
                {
                    _selectedAnnotationIds.Clear();
                }

                _selectionRectangleStart = point;
                _selectionRectangleEnd = point;
                _addRectangleSelection = control;
            }
            else
            {
                if (!control)
                {
                    _selectedAnnotationIds.Clear();
                }

                _selectedAnnotationIds.Add(hit.Id);
                BeginSelectionMove();
            }

            ApplyToolOptions();
            EditorCanvas.Invalidate();
            EditorCanvas.CapturePointer(args.Pointer);
            return;
        }

        if (_tool == ToolMode.Text)
        {
            var hit = AnnotationHitTester.HitTest(
                CurrentPage.Annotations,
                point,
                18 / Math.Max(_editorImageRect.Width, 1));
            if (hit is TextAnnotation text)
            {
                _pointerStart = null;
                SelectOnly(text.Id);
                ApplyToolOptions();
                BeginInlineTextEdit(
                    text.Id,
                    text.Text,
                    text.Bounds,
                    text.Transform);
                return;
            }

            EditorCanvas.CapturePointer(args.Pointer);
            return;
        }

        CheckpointInteraction();
        var settings = _toolSettings[_tool];
        Annotation annotation = _tool switch
        {
            ToolMode.Pen or ToolMode.Highlighter =>
                new FreehandAnnotation
                {
                    ColorArgb = settings.Color,
                    StrokeWidth = settings.Thickness,
                    IsHighlighter = _tool == ToolMode.Highlighter,
                    Filled = _tool == ToolMode.Pen && settings.Filled,
                    Points = [point],
                },
            _ => new ShapeAnnotation
            {
                ColorArgb = settings.Color,
                StrokeWidth = settings.Thickness,
                Shape = ToShapeKind(_tool),
                Start = point,
                End = point,
                Filled = settings.Filled,
            },
        };
        CurrentPage.Annotations.Add(annotation);
        _draftAnnotationId = annotation.Id;
        SelectOnly(annotation.Id);
        ApplyToolOptions();
        EditorCanvas.CapturePointer(args.Pointer);
        EditorCanvas.Invalidate();
    }

    private void EditorCanvas_PointerMoved(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (HandleNavigationMoved(args))
        {
            return;
        }

        if (_pointerStart is null
            || !TryGetPagePoint(
                args,
                out var point,
                allowSelectionMargin: _tool == ToolMode.Select))
        {
            return;
        }

        _pointerMoved |= PageDistancePixels(
            _pointerStart.Value,
            point) >= 3;
        if (_tool == ToolMode.Select)
        {
            if (_selectionRectangleStart is not null)
            {
                _selectionRectangleEnd = point;
                EditorCanvas.Invalidate();
                return;
            }

            if (!_pointerMoved)
            {
                return;
            }

            if (!_rotatingSelection
                && !_resizingSelection
                && _dragStartTransforms is null)
            {
                return;
            }

            CheckpointInteraction();
            if (_rotatingSelection
                && _rotationStartTransform is AffineTransform rotationStart
                && SelectedAnnotation is Annotation rotating)
            {
                var angle = PageAngle(_rotationCenter, point);
                var delta = MathF.Atan2(
                    MathF.Sin(angle - _rotationStartAngle),
                    MathF.Cos(angle - _rotationStartAngle));
                var operation = AnnotationGeometry.PageRotation(
                    delta,
                    _rotationCenter,
                    EditorPageWidthPoints(),
                    EditorPageHeightPoints());
                ReplaceAnnotation(
                    rotating.Id,
                    annotation => WithTransform(
                        annotation,
                        rotationStart.Matrix * operation));
            }
            else if (_resizingSelection
                && _resizeStartAnnotation is Annotation resizeStart
                && Matrix3x2.Invert(
                    resizeStart.Transform.Matrix,
                    out var inverse))
            {
                var local = NormalizedPoint.From(
                    Vector2.Transform(point.Vector, inverse));
                var resized = AnnotationGeometry.ResizeFromCorner(
                    resizeStart,
                    _resizeCornerIndex,
                    local,
                    EditorPageWidthPoints(),
                    EditorPageHeightPoints());
                ReplaceAnnotation(resizeStart.Id, _ => resized);
            }
            else if (_dragStartTransforms is not null)
            {
                var offset = point.Vector - _pointerStart.Value.Vector;
                foreach (var (id, transform) in _dragStartTransforms)
                {
                    var matrix = transform.Matrix
                        * Matrix3x2.CreateTranslation(offset);
                    ReplaceAnnotation(
                        id,
                        annotation => WithTransform(annotation, matrix));
                }
            }
        }
        else if (_draftAnnotationId is Guid draft)
        {
            ReplaceAnnotation(draft, annotation => annotation switch
            {
                FreehandAnnotation ink => ink with
                {
                    Points = AppendPoint(ink.Points, point),
                },
                ShapeAnnotation shape => shape with
                {
                    End = shape.Shape is ShapeKind.Checkmark
                        or ShapeKind.Cross
                            ? ConstrainToSquare(shape.Start, point)
                            : point,
                },
                _ => annotation,
            });
        }

        EditorCanvas.Invalidate();
    }

    private void EditorCanvas_PointerReleased(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (HandleNavigationReleased(args))
        {
            return;
        }

        if (_pointerStart is null)
        {
            return;
        }

        EditorCanvas.ReleasePointerCapture(args.Pointer);
        CompleteDraftAnnotation();
        if (_pointerMoved
            && _selectionRectangleStart is NormalizedPoint rectangleStart
            && _selectionRectangleEnd is NormalizedPoint rectangleEnd)
        {
            SelectByRectangle(
                NormalizedRect.FromPoints(rectangleStart, rectangleEnd),
                _addRectangleSelection);
        }

        if (!_pointerMoved
            && _toggleSelectionOnClick is Guid toggleId)
        {
            _selectedAnnotationIds.Remove(toggleId);
        }

        var changed = _interactionSnapshot is not null;
        var textStart = _pointerStart.Value;
        var textEnd = default(NormalizedPoint);
        var hasTextEnd = _tool == ToolMode.Text
            && TryGetPagePoint(args, out textEnd);
        _pointerStart = null;
        _draftAnnotationId = null;
        ResetSelectionInteraction();
        if (_tool == ToolMode.Text
            && hasTextEnd)
        {
            BeginNewTextEdit(textStart, textEnd);
        }

        if (changed)
        {
            CommitInteractionHistory();
            MarkChanged();
        }
        else
        {
            _interactionSnapshot = null;
        }

        ApplyToolOptions();
        EditorCanvas.Invalidate();
    }

    private void BeginNewTextEdit(
        NormalizedPoint start,
        NormalizedPoint end)
    {
        var bounds = NormalizedRect.FromPoints(start, end);
        if (bounds.Width < 0.08f || bounds.Height < 0.04f)
        {
            bounds = new NormalizedRect(
                start.X,
                start.Y,
                Math.Min(0.24f, 1 - start.X),
                Math.Min(0.07f, 1 - start.Y));
        }

        BeginInlineTextEdit(
            null,
            string.Empty,
            bounds,
            AffineTransform.Identity);
    }

    private void EditorCanvas_DoubleTapped(
        object sender,
        DoubleTappedRoutedEventArgs args)
    {
        if (_project.Pages.Count == 0
            || _tool is not (ToolMode.Select or ToolMode.Text))
        {
            return;
        }

        var position = ToSkia(args.GetPosition(EditorCanvas));
        if (!_editorImageRect.Contains(position))
        {
            return;
        }

        var point = ToNormalized(position, _editorImageRect);
        var hit = AnnotationHitTester.HitTest(
            CurrentPage.Annotations,
            point,
            18 / Math.Max(_editorImageRect.Width, 1));
        if (hit is not TextAnnotation text)
        {
            return;
        }

        SelectOnly(text.Id);
        ApplyToolOptions();
        BeginInlineTextEdit(
            text.Id,
            text.Text,
            text.Bounds,
            text.Transform);
        args.Handled = true;
    }

    private void BeginInlineTextEdit(
        Guid? annotationId,
        string text,
        NormalizedRect bounds,
        AffineTransform transform)
    {
        CancelInlineTextEdit();
        _textEditAnnotationId = annotationId;
        _textEditBounds = bounds;
        _textEditTransform = transform;
        InlineTextEditor.Text = text;
        InlineTextEditor.Visibility = Visibility.Visible;
        PositionInlineTextEditor(EditorPageWidthPoints());
        EditorCanvas.Invalidate();
        DispatcherQueue.TryEnqueue(() =>
        {
            if (InlineTextEditor.Visibility == Visibility.Visible)
            {
                InlineTextEditor.Focus(FocusState.Programmatic);
                InlineTextEditor.SelectAll();
            }
        });
    }

    private void PositionInlineTextEditor(float widthPoints)
    {
        if (InlineTextEditor.Visibility != Visibility.Visible
            || _editorImageRect.IsEmpty)
        {
            return;
        }

        var corners = BoundsCorners(_textEditBounds)
            .Select(_textEditTransform.Apply)
            .ToArray();
        var left = _editorImageRect.Left
            + corners.Min(point => point.X) * _editorImageRect.Width;
        var top = _editorImageRect.Top
            + corners.Min(point => point.Y) * _editorImageRect.Height;
        var right = _editorImageRect.Left
            + corners.Max(point => point.X) * _editorImageRect.Width;
        var bottom = _editorImageRect.Top
            + corners.Max(point => point.Y) * _editorImageRect.Height;
        Canvas.SetLeft(InlineTextEditor, left);
        Canvas.SetTop(InlineTextEditor, top);
        InlineTextEditor.Width = Math.Max(48, right - left);
        InlineTextEditor.Height = Math.Max(40, bottom - top);
        InlineTextEditor.FontFamily = new FontFamily(SelectedFontFamily());
        InlineTextEditor.FontSize = SelectedFontSize()
            * _editorImageRect.Width
            / Math.Max(widthPoints, 1);
        InlineTextEditor.Foreground = new SolidColorBrush(
            ToWindowsColor(SelectedTextColor()));
    }

    private void InlineTextEditor_LostFocus(
        object sender,
        RoutedEventArgs args)
    {
        if (!_closingTextEditor)
        {
            CommitInlineTextEdit();
        }
    }

    private void InlineTextEditor_KeyDown(
        object sender,
        KeyRoutedEventArgs args)
    {
        if (args.Key == VirtualKey.Enter
            && !IsKeyDown(VirtualKey.Shift))
        {
            args.Handled = true;
            CommitInlineTextEdit();
            EditorCanvas.Focus(FocusState.Programmatic);
            return;
        }

        if (args.Key != VirtualKey.Escape)
        {
            return;
        }

        args.Handled = true;
        CancelInlineTextEdit();
        EditorCanvas.Focus(FocusState.Programmatic);
    }

    private void CommitInlineTextEdit()
    {
        if (InlineTextEditor.Visibility != Visibility.Visible)
        {
            return;
        }

        var text = InlineTextEditor.Text.Trim();
        if (string.IsNullOrWhiteSpace(text))
        {
            CancelInlineTextEdit();
            EditorCanvas.Invalidate();
            return;
        }

        _history.Checkpoint(_project);
        var fontFamily = SelectedFontFamily();
        var fontSize = SelectedFontSize();
        Guid annotationId;
        if (_textEditAnnotationId is Guid existingId
            && CurrentPage.Annotations.FirstOrDefault(
                annotation => annotation.Id == existingId)
                is TextAnnotation existing)
        {
            annotationId = existingId;
            ReplaceAnnotation(existingId, annotation => existing with
            {
                Text = text,
                FontFamily = fontFamily,
                FontSize = fontSize,
            });
        }
        else
        {
            var settings = _toolSettings[ToolMode.Text];
            var annotation = new TextAnnotation
            {
                ColorArgb = settings.Color,
                StrokeWidth = 3,
                Text = text,
                Bounds = _textEditBounds,
                FontFamily = fontFamily,
                FontSize = fontSize,
            };
            CurrentPage.Annotations.Add(annotation);
            annotationId = annotation.Id;
        }

        CloseInlineTextEditor();
        SelectOnly(annotationId);
        MarkChanged();
        ApplyToolOptions();
        EditorCanvas.Invalidate();
    }

    private void CancelInlineTextEdit()
    {
        if (!_controlsReady
            || InlineTextEditor.Visibility != Visibility.Visible)
        {
            _textEditAnnotationId = null;
            return;
        }

        CloseInlineTextEditor();
        EditorCanvas.Invalidate();
    }

    private void CloseInlineTextEditor()
    {
        _closingTextEditor = true;
        InlineTextEditor.Visibility = Visibility.Collapsed;
        InlineTextEditor.Text = string.Empty;
        _textEditAnnotationId = null;
        _closingTextEditor = false;
    }

    private bool TryBeginSelectionResize(
        NormalizedPoint point,
        bool force)
    {
        if (SelectedAnnotation is not Annotation selected)
        {
            return false;
        }

        var corners = AnnotationGeometry.BoundsCorners(selected)
            .Select(selected.Transform.Apply)
            .ToArray();
        var closest = corners
            .Select((corner, index) => new
            {
                Index = index,
                Distance = PageDistancePixels(corner, point),
            })
            .MinBy(candidate => candidate.Distance);
        if (closest is null || !force && closest.Distance > 28)
        {
            return false;
        }

        _resizeStartAnnotation = selected;
        _resizeCornerIndex = closest.Index;
        _resizingSelection = true;
        return true;
    }

    private bool TryBeginSelectionRotation(NormalizedPoint point)
    {
        if (SelectedAnnotation is not Annotation selected)
        {
            return false;
        }

        var handle = RotationHandlePoint(selected);
        if (PageDistancePixels(handle, point) > 28)
        {
            return false;
        }

        _rotationCenter = AnnotationCenter(selected);
        _rotationStartAngle = PageAngle(_rotationCenter, point);
        _rotationStartTransform = selected.Transform;
        _rotatingSelection = true;
        return true;
    }

    private void BeginSelectionMove()
    {
        _dragStartTransforms = CurrentPage.Annotations
            .Where(annotation =>
                _selectedAnnotationIds.Contains(annotation.Id))
            .ToDictionary(
                annotation => annotation.Id,
                annotation => annotation.Transform);
    }

    private void SelectByRectangle(
        NormalizedRect rectangle,
        bool add)
    {
        if (!add)
        {
            _selectedAnnotationIds.Clear();
        }

        foreach (var annotation in CurrentPage.Annotations
            .Where(annotation =>
                AnnotationGeometry.Intersects(annotation, rectangle)))
        {
            _selectedAnnotationIds.Add(annotation.Id);
        }
    }

    private void ResetSelectionInteraction()
    {
        _dragStartTransforms = null;
        _resizeStartAnnotation = null;
        _resizingSelection = false;
        _rotationStartTransform = null;
        _rotatingSelection = false;
        _selectionRectangleStart = null;
        _selectionRectangleEnd = null;
        _toggleSelectionOnClick = null;
    }

    private void CheckpointInteraction()
    {
        if (_interactionSnapshot is not null)
        {
            return;
        }

        _interactionSnapshot = ProjectJson.Clone(_project);
    }

    private void CommitInteractionHistory()
    {
        if (_interactionSnapshot is null)
        {
            return;
        }

        _history.Checkpoint(_interactionSnapshot);
        _interactionSnapshot = null;
    }

    private void CancelEditorInteraction()
    {
        if (_interactionSnapshot is not null)
        {
            _project = _interactionSnapshot;
            _interactionSnapshot = null;
        }

        _pointerStart = null;
        _draftAnnotationId = null;
        ResetSelectionInteraction();
        _pointerMoved = false;
        _selectedAnnotationIds.RemoveWhere(id =>
            CurrentPage.Annotations.All(annotation => annotation.Id != id));

        ApplyToolOptions();
        EditorCanvas.Invalidate();
    }

    private bool HandleNavigationPressed(PointerRoutedEventArgs args)
    {
        if (_tool != ToolMode.Navigate)
        {
            return false;
        }

        _navigationPoints[args.Pointer.PointerId] =
            args.GetCurrentPoint(EditorCanvas).Position;
        BeginNavigationGesture();
        EditorCanvas.CapturePointer(args.Pointer);
        args.Handled = true;
        return true;
    }

    private bool HandleNavigationMoved(PointerRoutedEventArgs args)
    {
        if (_tool != ToolMode.Navigate
            || !_navigationPoints.ContainsKey(args.Pointer.PointerId))
        {
            return false;
        }

        _navigationPoints[args.Pointer.PointerId] =
            args.GetCurrentPoint(EditorCanvas).Position;
        var points = _navigationPoints.Values.Take(2).ToArray();
        if (points.Length >= 2)
        {
            var distance = PointDistance(points[0], points[1]);
            var scale = distance
                / Math.Max(_gestureStartDistance, 1);
            var zoom = Math.Clamp(
                _gestureStartZoom * (float)scale,
                1,
                8);
            var center = PointMidpoint(points[0], points[1]);
            _zoom = zoom;
            _pan = ViewportNavigation.PinchPan(
                _editorViewportSize,
                ToVector(_gestureStartCenter),
                ToVector(center),
                _gestureStartPan,
                _gestureStartZoom,
                _zoom);
        }
        else
        {
            var offset = ToVector(points[0])
                - ToVector(_gestureStartCenter);
            _pan = _gestureStartPan + offset;
        }

        ClampPan();
        EditorCanvas.Invalidate();
        args.Handled = true;
        return true;
    }

    private bool HandleNavigationReleased(PointerRoutedEventArgs args)
    {
        if (_tool != ToolMode.Navigate
            || !_navigationPoints.Remove(args.Pointer.PointerId))
        {
            return false;
        }

        EditorCanvas.ReleasePointerCapture(args.Pointer);
        if (_navigationPoints.Count > 0)
        {
            BeginNavigationGesture();
        }

        args.Handled = true;
        return true;
    }

    private void BeginNavigationGesture()
    {
        var points = _navigationPoints.Values.Take(2).ToArray();
        _gestureStartPan = _pan;
        _gestureStartZoom = _zoom;
        _gestureStartCenter = points.Length >= 2
            ? PointMidpoint(points[0], points[1])
            : points[0];
        _gestureStartDistance = points.Length >= 2
            ? PointDistance(points[0], points[1])
            : 0;
    }

    private void EditorCanvas_PointerCanceled(
        object sender,
        PointerRoutedEventArgs args)
    {
        if (HandleNavigationReleased(args))
        {
            return;
        }

        CancelEditorInteraction();
        EditorCanvas.ReleasePointerCapture(args.Pointer);
        args.Handled = true;
    }

    private void CompleteDraftAnnotation()
    {
        if (_draftAnnotationId is not Guid id
            || CurrentPage.Annotations.FirstOrDefault(
                annotation => annotation.Id == id)
                is not ShapeAnnotation shape
            || PageDistancePixels(shape.Start, shape.End) >= 12)
        {
            return;
        }

        var end = DefaultShapeEnd(shape);
        ReplaceAnnotation(id, annotation => shape with { End = end });
    }

    private NormalizedPoint DefaultShapeEnd(ShapeAnnotation shape)
    {
        var width = Math.Max(_editorImageRect.Width, 1);
        var height = Math.Max(_editorImageRect.Height, 1);
        var defaultWidth = shape.Shape is ShapeKind.Checkmark
            or ShapeKind.Cross
            ? 28
            : 64;
        var defaultHeight = shape.Shape is ShapeKind.Checkmark
            or ShapeKind.Cross
            ? 28
            : 48;
        var target = new NormalizedPoint(
            shape.Start.X + defaultWidth / width,
            shape.Start.Y + defaultHeight / height).Clamp();
        if (shape.Shape is ShapeKind.Checkmark or ShapeKind.Cross)
        {
            return ConstrainToSquare(shape.Start, target);
        }

        if (shape.Shape is ShapeKind.Line or ShapeKind.Arrow)
        {
            target = target with { Y = shape.Start.Y };
        }

        return target;
    }

    private NormalizedPoint ConstrainToSquare(
        NormalizedPoint start,
        NormalizedPoint end)
    {
        var width = Math.Max(_editorImageRect.Width, 1);
        var height = Math.Max(_editorImageRect.Height, 1);
        var deltaX = (end.X - start.X) * width;
        var deltaY = (end.Y - start.Y) * height;
        var directionX = deltaX < 0 ? -1 : 1;
        var directionY = deltaY < 0 ? -1 : 1;
        var availableX = directionX > 0
            ? (1 - start.X) * width
            : start.X * width;
        var availableY = directionY > 0
            ? (1 - start.Y) * height
            : start.Y * height;
        var size = Math.Min(
            Math.Max(Math.Abs(deltaX), Math.Abs(deltaY)),
            Math.Min(availableX, availableY));
        return new NormalizedPoint(
            start.X + directionX * size / width,
            start.Y + directionY * size / height);
    }

    private float PageDistancePixels(
        NormalizedPoint first,
        NormalizedPoint second)
    {
        var x = (first.X - second.X) * _editorImageRect.Width;
        var y = (first.Y - second.Y) * _editorImageRect.Height;
        return MathF.Sqrt(x * x + y * y);
    }

    private void ToolButton_Click(object sender, RoutedEventArgs args)
    {
        if (sender is not ToggleButton selected
            || selected.Tag is not string name
            || !Enum.TryParse(name, out ToolMode tool))
        {
            return;
        }

        CommitInlineTextEdit();
        _navigationPoints.Clear();
        foreach (var button in ToolButtonGrid.Children
            .OfType<ToggleButton>())
        {
            button.IsChecked = ReferenceEquals(button, selected);
        }

        _tool = tool;
        if (tool != ToolMode.Select)
        {
            _selectedAnnotationIds.Clear();
        }

        ApplyToolOptions();
        EditorCanvas.Invalidate();
    }

    private void ColorButton_Click(object sender, RoutedEventArgs args)
    {
        if (!ColorPanel.IsHitTestVisible
            || sender is not Button { Tag: string value }
            || !uint.TryParse(
                value,
                NumberStyles.HexNumber,
                CultureInfo.InvariantCulture,
                out var color))
        {
            return;
        }

        if (_tool != ToolMode.Select)
        {
            _toolSettings[_tool] = _toolSettings[_tool] with
            {
                Color = color,
            };
        }

        ChangeSelected(annotation => annotation with
        {
            ColorArgb = color,
        });
        UpdateColorSelection(color);
        if (InlineTextEditor.Visibility == Visibility.Visible)
        {
            InlineTextEditor.Foreground = new SolidColorBrush(
                ToWindowsColor(color));
        }
    }

    private void ThicknessSlider_ValueChanged(
        object sender,
        RangeBaseValueChangedEventArgs args)
    {
        if (!_controlsReady || _updatingToolOptions)
        {
            return;
        }

        if (_tool != ToolMode.Select)
        {
            _toolSettings[_tool] = _toolSettings[_tool] with
            {
                Thickness = (float)args.NewValue,
            };
        }

        ChangeSelected(annotation => annotation with
        {
            StrokeWidth = annotation is TextAnnotation
                ? annotation.StrokeWidth
                : (float)args.NewValue,
        });
    }

    private void FilledToggle_Toggled(
        object sender,
        RoutedEventArgs args)
    {
        if (!_controlsReady || _updatingToolOptions)
        {
            return;
        }

        if (_tool != ToolMode.Select)
        {
            _toolSettings[_tool] = _toolSettings[_tool] with
            {
                Filled = FilledToggle.IsOn,
            };
        }

        ChangeSelected(annotation => annotation switch
        {
            ShapeAnnotation
            {
                Shape: ShapeKind.Rectangle or ShapeKind.Ellipse,
            } shape => shape with
            {
                Filled = FilledToggle.IsOn,
            },
            FreehandAnnotation { IsHighlighter: false } ink => ink with
            {
                Filled = FilledToggle.IsOn,
            },
            _ => annotation,
        });
    }

    private void FontFamily_SelectionChanged(
        object sender,
        SelectionChangedEventArgs args)
    {
        if (!_controlsReady || _updatingToolOptions)
        {
            return;
        }

        var family = SelectedFontFamily();
        _toolSettings[ToolMode.Text] = _toolSettings[ToolMode.Text] with
        {
            FontFamily = family,
        };
        if (InlineTextEditor.Visibility == Visibility.Visible)
        {
            InlineTextEditor.FontFamily = new FontFamily(family);
            return;
        }

        ChangeSelected(annotation => annotation is TextAnnotation text
            ? text with { FontFamily = family }
            : annotation);
    }

    private void FontSize_ValueChanged(
        NumberBox sender,
        NumberBoxValueChangedEventArgs args)
    {
        if (!_controlsReady || _updatingToolOptions)
        {
            return;
        }

        var size = SelectedFontSize();
        _toolSettings[ToolMode.Text] = _toolSettings[ToolMode.Text] with
        {
            FontSize = size,
        };
        if (InlineTextEditor.Visibility == Visibility.Visible)
        {
            PositionInlineTextEditor(EditorPageWidthPoints());
            return;
        }

        ChangeSelected(annotation => annotation is TextAnnotation text
            ? text with { FontSize = size }
            : annotation);
    }

    private void ApplyToolOptions()
    {
        if (!_controlsReady)
        {
            return;
        }

        _updatingToolOptions = true;
        var selections = SelectedAnnotations;
        var selected = selections.FirstOrDefault();
        var hasSelection = selections.Count > 0;
        var settings = selected is null
            ? _toolSettings[_tool]
            : ToolSettings.FromAnnotation(selected);
        ThicknessSlider.Value = Math.Clamp(
            settings.Thickness,
            (float)ThicknessSlider.Minimum,
            (float)ThicknessSlider.Maximum);
        FilledToggle.IsOn = settings.Filled;
        SelectComboItem(FontFamilyPicker, settings.FontFamily);
        FontSizePicker.Value = settings.FontSize;
        UpdateColorSelection(settings.Color);

        var supportsThickness = hasSelection
            ? selections.Any(annotation => annotation is not TextAnnotation)
            : _tool is not (
                ToolMode.Navigate or ToolMode.Select or ToolMode.Text);
        var supportsFill = hasSelection
            ? selections.Any(SupportsFill)
            : _tool is ToolMode.Pen
                or ToolMode.Rectangle
                or ToolMode.Ellipse;
        var supportsFont = hasSelection
            ? selections.All(annotation => annotation is TextAnnotation)
            : _tool == ToolMode.Text;
        var supportsColor = hasSelection
            || _tool is not (ToolMode.Navigate or ToolMode.Select);
        ThicknessSlider.IsEnabled = supportsThickness;
        ThicknessLabel.Opacity = supportsThickness ? 1 : 0.45;
        FilledToggle.IsEnabled = supportsFill;
        FillPanel.Opacity = supportsFill ? 1 : 0.45;
        FontPanel.Visibility = supportsFont
            ? Visibility.Visible
            : Visibility.Collapsed;
        ColorPanel.IsHitTestVisible = supportsColor;
        ColorPanel.Opacity = supportsColor ? 1 : 0.45;
        foreach (var button in ColorPanel.Children.OfType<Button>())
        {
            button.IsTabStop = supportsColor;
        }

        NavigationBar.Visibility = _tool == ToolMode.Navigate
            ? Visibility.Visible
            : Visibility.Collapsed;
        foreach (var button in TransformPanel.Children.OfType<Button>())
        {
            button.IsEnabled = hasSelection;
        }

        DeleteObjectButton.IsEnabled = hasSelection;
        DeleteAllObjectsButton.IsEnabled = _project.Pages.Count > 0
            && CurrentPage.Annotations.Count > 0;
        _updatingToolOptions = false;
    }

    private void UpdateColorSelection(uint color)
    {
        foreach (var button in ColorPanel.Children.OfType<Button>())
        {
            var selected = button.Tag is string tag
                && uint.TryParse(
                    tag,
                    NumberStyles.HexNumber,
                    CultureInfo.InvariantCulture,
                    out var value)
                && value == color;
            button.BorderThickness = new Thickness(selected ? 3 : 1);
            button.BorderBrush = new SolidColorBrush(
                selected
                    ? Windows.UI.Color.FromArgb(255, 0, 164, 239)
                    : Windows.UI.Color.FromArgb(102, 127, 127, 127));
        }
    }

    private static void SelectComboItem(ComboBox comboBox, string value)
    {
        var match = comboBox.Items
            .OfType<ComboBoxItem>()
            .Select((item, index) => new { Item = item, Index = index })
            .FirstOrDefault(candidate => string.Equals(
                candidate.Item.Content?.ToString(),
                value,
                StringComparison.Ordinal));
        if (match is not null)
        {
            comboBox.SelectedIndex = match.Index;
        }
    }

    private void RotateObjectButton_Click(
        object sender,
        RoutedEventArgs args) => RotateSelected(MathF.PI / 12);

    private void SmallerObjectButton_Click(
        object sender,
        RoutedEventArgs args) => ResizeSelected(0.9f);

    private void LargerObjectButton_Click(
        object sender,
        RoutedEventArgs args) => ResizeSelected(1.1f);

    private void RotateSelected(float radians)
    {
        var selections = SelectedAnnotations;
        if (selections.Count == 0)
        {
            return;
        }

        _history.Checkpoint(_project);
        foreach (var selected in selections)
        {
            var operation = AnnotationGeometry.PageRotation(
                radians,
                AnnotationCenter(selected),
                EditorPageWidthPoints(),
                EditorPageHeightPoints());
            ReplaceAnnotation(
                selected.Id,
                annotation => WithTransform(
                    annotation,
                    annotation.Transform.Matrix * operation));
        }

        MarkChanged();
        EditorCanvas.Invalidate();
    }

    private void ResizeSelected(float factor)
    {
        var selections = SelectedAnnotations;
        if (selections.Count == 0)
        {
            return;
        }

        _history.Checkpoint(_project);
        foreach (var selected in selections)
        {
            var resized = AnnotationGeometry.ResizeByFactor(
                selected,
                factor);
            ReplaceAnnotation(selected.Id, _ => resized);
        }

        MarkChanged();
        EditorCanvas.Invalidate();
    }

    private void DeleteObjectButton_Click(
        object sender,
        RoutedEventArgs args) => DeleteSelectedAnnotations();

    private void DeleteSelectedAnnotations()
    {
        if (_selectedAnnotationIds.Count == 0)
        {
            return;
        }

        _history.Checkpoint(_project);
        CurrentPage.Annotations.RemoveAll(annotation =>
            _selectedAnnotationIds.Contains(annotation.Id));
        _selectedAnnotationIds.Clear();
        MarkChanged();
        ApplyToolOptions();
        EditorCanvas.Invalidate();
    }

    private async void DeleteAllObjectsButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (CurrentPage.Annotations.Count == 0)
        {
            return;
        }

        var dialog = new ContentDialog
        {
            Title = "Delete all annotations?",
            Content = "Every annotation on this page will be removed.",
            PrimaryButtonText = "Delete all",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }

        _history.Checkpoint(_project);
        CurrentPage.Annotations.Clear();
        _selectedAnnotationIds.Clear();
        MarkChanged();
        ApplyToolOptions();
        EditorCanvas.Invalidate();
    }

    private void ZoomOutButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        SetEditorZoom(_zoom / 1.25f);
    }

    private void FitButton_Click(object sender, RoutedEventArgs args)
    {
        SetEditorZoom(1, resetPan: true);
    }

    private void ZoomInButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        SetEditorZoom(_zoom * 1.25f);
    }

    private void ActualSizeButton_Click(
        object sender,
        RoutedEventArgs args)
    {
        if (_editorBackground is null
            || _editorViewportSize.X <= 0
            || _editorViewportSize.Y <= 0)
        {
            return;
        }

        var fullView = FitRect(
            _editorViewportSize.X,
            _editorViewportSize.Y,
            _editorBackground.Width,
            _editorBackground.Height,
            1);
        var fitScale = fullView.Width / _editorBackground.Width;
        var rasterScale = (float)Math.Max(
            XamlRoot.RasterizationScale,
            0.001);
        SetEditorZoom(
            1 / Math.Max(fitScale * rasterScale, 0.001f),
            resetPan: true);
    }

    private void SetEditorZoom(float zoom, bool resetPan = false)
    {
        _zoom = Math.Clamp(zoom, 1, 8);
        if (resetPan)
        {
            _pan = Vector2.Zero;
        }

        ClampPan();
        EditorCanvas.Invalidate();
    }

    private async void PageList_SelectionChanged(
        object sender,
        SelectionChangedEventArgs args)
    {
        if (_updatingPageList
            || _reorderingPages
            || PageList.SelectedIndex < 0
            || PageList.SelectedIndex == _pageIndex)
        {
            return;
        }

        _pageIndex = PageList.SelectedIndex;
        await LoadActivePageAsync();
    }

    private void PageList_DragItemsStarting(
        object sender,
        DragItemsStartingEventArgs args)
    {
        _reorderingPages = true;
        _activePageBeforeReorder = CurrentPage.Id;
    }

    private void PageList_DragItemsCompleted(
        object sender,
        DragItemsCompletedEventArgs args)
    {
        var orderedIds = PageList.Items
            .OfType<PageListItem>()
            .Select(item => item.PageId)
            .ToList();
        _reorderingPages = false;
        var selectedPageId = _activePageBeforeReorder
            ?? _project.Pages[_pageIndex].Id;
        _activePageBeforeReorder = null;
        if (orderedIds.Count != _project.Pages.Count
            || orderedIds.SequenceEqual(
                _project.Pages.Select(page => page.Id)))
        {
            _updatingPageList = true;
            PageList.SelectedIndex = _project.Pages.FindIndex(
                page => page.Id == selectedPageId);
            _updatingPageList = false;
            return;
        }

        var pages = _project.Pages.ToDictionary(page => page.Id);
        _history.Checkpoint(_project);
        _project.Pages.Clear();
        _project.Pages.AddRange(orderedIds.Select(id => pages[id]));
        _pageIndex = _project.Pages.FindIndex(
            page => page.Id == selectedPageId);
        _updatingPageList = true;
        PageList.SelectedIndex = _pageIndex;
        _updatingPageList = false;
        MarkChanged();
        SetStatus($"Page {_pageIndex + 1} of {_project.Pages.Count}");
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

    private async void RootGrid_KeyDown(
        object sender,
        KeyRoutedEventArgs args)
    {
        if (args.Key == VirtualKey.Delete
            && _screen == AppScreen.Editor
            && InlineTextEditor.Visibility != Visibility.Visible)
        {
            args.Handled = true;
            DeleteSelectedAnnotations();
            return;
        }

        var control = InputKeyboardSource.GetKeyStateForCurrentThread(
            VirtualKey.Control);
        if (!control.HasFlag(CoreVirtualKeyStates.Down))
        {
            return;
        }

        if (args.Key == VirtualKey.Z)
        {
            args.Handled = true;
            await UndoAsync();
        }
        else if (args.Key == VirtualKey.Y)
        {
            args.Handled = true;
            await RedoAsync();
        }
    }

    private async Task UndoAsync()
    {
        if (!_history.TryUndo(_project, out var project))
        {
            return;
        }

        await ApplyHistoryStateAsync(project);
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

        await ApplyHistoryStateAsync(project);
    }

    private async Task ApplyHistoryStateAsync(FastFillProject project)
    {
        CancelInlineTextEdit();
        var previousPage = _project.Pages.Count > 0
            ? CurrentPage
            : null;
        var previousPageId = previousPage?.Id;
        _project = project;
        var matchingIndex = previousPageId is Guid id
            ? _project.Pages.FindIndex(page => page.Id == id)
            : -1;
        _pageIndex = _project.Pages.Count == 0
            ? 0
            : matchingIndex >= 0
                ? matchingIndex
                : Math.Clamp(_pageIndex, 0, _project.Pages.Count - 1);
        MarkChanged();
        ShowEditor();
        if (previousPage is not null
            && _project.Pages.Count > 0
            && PageRasterEquals(previousPage, CurrentPage))
        {
            _selectedAnnotationIds.RemoveWhere(selectedId =>
                CurrentPage.Annotations.All(
                    annotation => annotation.Id != selectedId));
            ApplyToolOptions();
            EditorCanvas.Invalidate();
            SetStatus($"Page {_pageIndex + 1} of {_project.Pages.Count}");
            return;
        }

        await LoadActivePageAsync();
    }

    private static bool PageRasterEquals(
        DocumentPage first,
        DocumentPage second) => first.Id == second.Id
        && first.SourceAsset == second.SourceAsset
        && first.Crop == second.Crop
        && first.RotationQuarterTurns == second.RotationQuarterTurns
        && first.Filter == second.Filter;

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
                Path.GetTempPath(),
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
        if (_selectedAnnotationIds.Count == 0)
        {
            return;
        }

        _history.Checkpoint(_project);
        foreach (var id in _selectedAnnotationIds)
        {
            ReplaceAnnotation(id, update);
        }

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

    private void ShowRecoverIfAvailable() =>
        RecoverButton.IsEnabled = FindLatestRecoveryManifest() is not null;

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
        out NormalizedPoint point,
        bool allowSelectionMargin = false)
    {
        var position = ToSkia(
            args.GetCurrentPoint(EditorCanvas).Position);
        const float selectionMargin = 36;
        var margin = allowSelectionMargin ? selectionMargin : 0;
        if (position.X < _editorImageRect.Left - margin
            || position.X > _editorImageRect.Right + margin
            || position.Y < _editorImageRect.Top - margin
            || position.Y > _editorImageRect.Bottom + margin)
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

    private void ClampPan()
    {
        if (_editorBackground is null
            || _editorViewportSize.X <= 0
            || _editorViewportSize.Y <= 0
            || _zoom <= 1)
        {
            _pan = Vector2.Zero;
            return;
        }

        var fullView = FitRect(
            _editorViewportSize.X,
            _editorViewportSize.Y,
            _editorBackground.Width,
            _editorBackground.Height,
            1);
        _pan = ViewportNavigation.ClampPan(
            _pan,
            _editorViewportSize,
            new Vector2(fullView.Width, fullView.Height),
            _zoom);
    }

    private static double PointDistance(Point first, Point second)
    {
        var x = first.X - second.X;
        var y = first.Y - second.Y;
        return Math.Sqrt(x * x + y * y);
    }

    private static Point PointMidpoint(Point first, Point second) => new(
        (first.X + second.X) / 2,
        (first.Y + second.Y) / 2);

    private static Vector2 ToVector(Point point) => new(
        (float)point.X,
        (float)point.Y);

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
        ToolMode.Cross => ShapeKind.Cross,
        _ => throw new ArgumentOutOfRangeException(nameof(tool)),
    };

    private float EditorPageWidthPoints() => _processedPage is null
        ? 612
        : _processedPage.Width / PdfExporter.DotsPerInch * 72;

    private float EditorPageHeightPoints() => _processedPage is null
        ? 792
        : _processedPage.Height / PdfExporter.DotsPerInch * 72;

    private string SelectedFontFamily() =>
        (FontFamilyPicker.SelectedItem as ComboBoxItem)
            ?.Content?.ToString()
        ?? _toolSettings[ToolMode.Text].FontFamily;

    private float SelectedFontSize()
    {
        return double.IsNaN(FontSizePicker.Value)
            ? _toolSettings[ToolMode.Text].FontSize
            : (float)FontSizePicker.Value;
    }

    private uint SelectedTextColor()
    {
        if (_textEditAnnotationId is Guid id
            && CurrentPage.Annotations.FirstOrDefault(
                annotation => annotation.Id == id) is TextAnnotation text)
        {
            return text.ColorArgb;
        }

        return SelectedAnnotations.FirstOrDefault()?.ColorArgb
            ?? _toolSettings[ToolMode.Text].Color;
    }

    private static Windows.UI.Color ToWindowsColor(uint color) =>
        Windows.UI.Color.FromArgb(
            (byte)(color >> 24),
            (byte)(color >> 16),
            (byte)(color >> 8),
            (byte)color);

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
        var bounds = AnnotationGeometry.Bounds(annotation);
        var localCenter = new Vector2(
            bounds.X + bounds.Width / 2,
            bounds.Y + bounds.Height / 2);
        return Vector2.Transform(localCenter, annotation.Transform.Matrix);
    }

    private NormalizedPoint RotationHandlePoint(Annotation annotation)
    {
        var bounds = AnnotationGeometry.Bounds(annotation);
        var local = new NormalizedPoint(
            bounds.X + bounds.Width / 2,
            bounds.Y
                - PageRenderer.RotationHandleOffset
                / Math.Max(_editorImageRect.Height, 1));
        return annotation.Transform.Apply(local);
    }

    private float PageAngle(
        Vector2 center,
        NormalizedPoint point) => MathF.Atan2(
        (point.Y - center.Y) * _editorImageRect.Height,
        (point.X - center.X) * _editorImageRect.Width);

    private static bool IsKeyDown(VirtualKey key) =>
        InputKeyboardSource.GetKeyStateForCurrentThread(key)
            .HasFlag(CoreVirtualKeyStates.Down);

    private static bool SupportsFill(Annotation annotation) =>
        annotation is FreehandAnnotation { IsHighlighter: false }
        or ShapeAnnotation
        {
            Shape: ShapeKind.Rectangle or ShapeKind.Ellipse,
        };

    private static NormalizedPoint[] BoundsCorners(
        NormalizedRect bounds) =>
    [
        new(bounds.X, bounds.Y),
        new(bounds.Right, bounds.Y),
        new(bounds.Right, bounds.Bottom),
        new(bounds.X, bounds.Bottom),
    ];

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

    private IReadOnlyList<Annotation> SelectedAnnotations =>
        _project.Pages.Count == 0
            ? []
            : CurrentPage.Annotations
                .Where(annotation =>
                    _selectedAnnotationIds.Contains(annotation.Id))
                .ToList();

    private Annotation? SelectedAnnotation =>
        _selectedAnnotationIds.Count == 1
            ? SelectedAnnotations.FirstOrDefault()
            : null;

    private NormalizedRect? SelectionRectangle =>
        _pointerMoved
            && _selectionRectangleStart is NormalizedPoint start
            && _selectionRectangleEnd is NormalizedPoint end
            ? NormalizedRect.FromPoints(start, end)
            : null;

    private void SelectOnly(Guid id)
    {
        _selectedAnnotationIds.Clear();
        _selectedAnnotationIds.Add(id);
    }

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
        Navigate,
        Select,
        Text,
        Pen,
        Highlighter,
        Line,
        Arrow,
        Rectangle,
        Ellipse,
        Checkmark,
        Cross,
    }

    private sealed record ToolSettings(
        uint Color,
        float Thickness,
        bool Filled,
        string FontFamily = "Segoe UI",
        float FontSize = 18)
    {
        public static ToolSettings FromAnnotation(Annotation annotation) =>
            annotation switch
            {
                TextAnnotation text => new(
                    text.ColorArgb,
                    text.StrokeWidth,
                    false,
                    text.FontFamily,
                    text.FontSize),
                FreehandAnnotation ink => new(
                    ink.ColorArgb,
                    ink.StrokeWidth,
                    ink.Filled),
                ShapeAnnotation shape => new(
                    shape.ColorArgb,
                    shape.StrokeWidth,
                    shape.Filled),
                _ => new(0xff111827, 3, false),
            };
    }

    private static Dictionary<ToolMode, ToolSettings>
        CreateToolSettings() => new()
    {
        [ToolMode.Navigate] = new(0xff111827, 3, false),
        [ToolMode.Select] = new(0xff111827, 3, false),
        [ToolMode.Text] = new(0xff111827, 3, false),
        [ToolMode.Pen] = new(0xff111827, 3, false),
        [ToolMode.Highlighter] = new(0xffffd800, 14, false),
        [ToolMode.Line] = new(0xff111827, 3, false),
        [ToolMode.Arrow] = new(0xff111827, 3, false),
        [ToolMode.Rectangle] = new(0xff111827, 3, false),
        [ToolMode.Ellipse] = new(0xff111827, 3, false),
        [ToolMode.Checkmark] = new(0xff16a34a, 3, false),
        [ToolMode.Cross] = new(0xffe11d48, 3, false),
    };

    private sealed record PageListItem(Guid PageId, string Label)
    {
        public override string ToString() => Label;
    }

    private sealed record PdfImportPage(
        byte[] Bytes,
        int Width,
        int Height);

    [ComImport]
    [Guid("3A3DCD6C-3EAB-43DC-BCDE-45671CE800C8")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IDataTransferManagerInterop
    {
        IntPtr GetForWindow(IntPtr appWindow, ref Guid interfaceId);

        void ShowShareUIForWindow(IntPtr appWindow);
    }
}
