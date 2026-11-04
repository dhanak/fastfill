// Browser preview stays in the DOM; only a confirmed PNG crosses into WASM.
const style = `
.ff-camera {
  color-scheme: light dark;
  --surface: #f6f7f8;
  --control: #eef0f2;
  --hover: #e1e5e9;
  --border: #b8bec5;
  --text: #23272d;
  width: 96vw;
  height: 96dvh;
  box-sizing: border-box;
  max-width: none;
  max-height: none;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  color: var(--text);
  font: 14px/1.35 "Fira Sans", system-ui, sans-serif;
  box-shadow: 0 18px 60px #0008;
}
@media (prefers-color-scheme: dark) {
  .ff-camera {
    --surface: #30343a;
    --control: #363b42;
    --hover: #454b54;
    --border: #545b65;
    --text: #f2f3f5;
  }
}
.ff-camera:not([open]) { display: none; }
.ff-camera[open] { display: block; }
.ff-camera::backdrop { background: #0007; }
.ff-camera button, .ff-camera select {
  min-height: 40px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--control);
  color: var(--text);
  font: inherit;
}
.ff-camera button { padding: 6px 12px; cursor: pointer; }
.ff-camera button:hover:not(:disabled) { background: var(--hover); }
.ff-camera select:hover:not(:disabled) { background: var(--hover); }
.ff-camera button:disabled { cursor: default; opacity: .45; }
.ff-camera button:focus-visible, .ff-camera select:focus-visible {
  outline: 2px solid #5966f2;
  outline-offset: 2px;
}
.ff-camera [data-action="capture"],
.ff-camera [data-action="use"],
.ff-camera [data-action="mirror"][aria-pressed="true"] {
  background: #5966f2;
  border-color: #5966f2;
  color: white;
}
.ff-camera [data-action="capture"]:hover:not(:disabled),
.ff-camera [data-action="use"]:hover:not(:disabled),
.ff-camera [data-action="mirror"][aria-pressed="true"]:hover {
  background: #4c58df;
}
.ff-camera-view {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: #17191d;
}
.ff-camera-view video, .ff-camera-view img {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 100%;
  height: 100%;
  object-fit: contain;
}
.ff-camera [hidden] { display: none !important; }
.ff-camera-view img { transform: translate(-50%, -50%); }
.ff-camera-view img.mirrored {
  transform: translate(-50%, -50%) scaleX(-1);
}
.ff-camera-outline {
  position: absolute;
  z-index: 1;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
.ff-camera-outline polygon {
  fill: #00aa6930;
  stroke: #00aa69;
  stroke-width: 2;
}
.ff-camera-outline circle {
  fill: #00aa69;
  stroke: white;
  stroke-width: 2;
}
.ff-camera-dock {
  position: absolute;
  z-index: 2;
  display: flex;
  gap: 8px;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--surface) 88%, transparent);
}
.ff-camera-dock.vertical { flex-direction: column; }
.ff-camera-dock.compact {
  display: grid;
  grid-template-columns: repeat(3, 40px);
}
.ff-camera-dock button, .ff-camera button.ff-camera-switch,
.ff-camera-settings button.icon {
  box-sizing: border-box;
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  min-height: 40px;
  padding: 0;
}
.ff-camera-dock .round { border-radius: 50%; }
.ff-camera-dock [data-action="capture"] { position: relative; }
.ff-camera-dock [data-action="capture"]::after {
  content: "";
  position: absolute;
  inset: -4px;
  border-radius: 50%;
  background: conic-gradient(#5966f2 var(--progress-angle, 0deg),
    #b4b9be 0);
  mask: radial-gradient(farthest-side, transparent calc(100% - 3px),
    black calc(100% - 2px));
  pointer-events: none;
}
.ff-camera-switch { position: absolute; z-index: 2; }
.ff-camera-icon {
  display: block;
  width: 24px;
  height: 24px;
  background: currentColor;
  mask: var(--icon) center / contain no-repeat;
}
.ff-camera-settings {
  position: absolute;
  z-index: 3;
  top: 12px;
  right: 12px;
  box-sizing: border-box;
  width: min(320px, calc(100% - 24px));
  max-height: calc(100% - 24px);
  overflow: auto;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  box-shadow: 0 10px 32px #0007;
}
.ff-camera-settings header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  font-size: 18px;
}
.ff-camera-settings label { display: block; margin: 8px 0 4px; }
.ff-camera-settings select { box-sizing: border-box; width: 100%; }
.ff-camera-settings p { margin: 8px 0; }
.ff-camera-settings [data-action="retry"] { width: 100%; }
.ff-camera-status {
  position: absolute;
  z-index: 1;
  left: 50%;
  bottom: 72px;
  max-width: calc(100% - 32px);
  padding: 7px 13px;
  border-radius: 20px;
  background: #17191dd9;
  color: white;
  text-align: center;
  transform: translateX(-50%);
  pointer-events: none;
}
@media (max-width: 600px) {
  .ff-camera { width: 100vw; height: 100dvh; border-radius: 0; }
}
`;

const icons = {
  cancel: "x", rotate: "rotate-cw",
  mirror: "triangles-centerline-dashed-vertical",
  capture: "camera", use: "check", pause: "pause", play: "play",
  retake: "undo-2", settings: "settings",
  switch: "switch-camera", close: "x",
};

function icon(action, label, extra = "") {
  const src = `./icons/${icons[action]}.svg`;
  return `<button data-action="${action}" type="button" ${extra}
    aria-label="${label}" title="${label}">
    <span class="ff-camera-icon" style="--icon:url('${src}')"
      aria-hidden="true"></span>
  </button>`;
}

function addStyle() {
  if (document.getElementById("ff-camera-style")) return;
  const node = document.createElement("style");
  node.id = "ff-camera-style";
  node.textContent = style;
  document.head.append(node);
}

function cameraError(error) {
  if (error?.name === "NotAllowedError") {
    return "Camera access denied. Allow it in browser site settings.";
  }
  if (error?.name === "NotFoundError") {
    return "No camera found. Connect a camera and retry.";
  }
  if (error?.name === "NotReadableError") {
    return "Camera is busy or unavailable. Close other camera apps and retry.";
  }
  return error?.message || String(error);
}

export async function capturePhoto(pixelBudget, analyze = null) {
  if (!globalThis.isSecureContext ||
      !navigator.mediaDevices?.getUserMedia) {
    throw "Camera needs HTTPS or localhost and browser camera support.";
  }
  addStyle();
  const dialog = document.createElement("dialog");
  dialog.className = "ff-camera";
  dialog.setAttribute("aria-label", "Camera capture");
  dialog.innerHTML = `
    <div class="ff-camera-view">
      <video autoplay muted playsinline></video>
      <img alt="Captured photo" hidden>
      <svg class="ff-camera-outline" aria-hidden="true"></svg>
      <div class="ff-camera-status" role="status">Starting camera…</div>
      ${icon("switch", "Switch camera", "class=\"ff-camera-switch\" disabled")}
      <div class="ff-camera-dock">
        ${icon("cancel", "Cancel")}
        ${icon("rotate", "Rotate 90°")}
        ${icon("mirror", "Mirror preview", "aria-pressed=\"false\"")}
        ${icon("capture", "Capture", "class=\"round\" disabled")}
        ${icon("use", "Use photo", "class=\"round\" hidden disabled")}
        ${icon("pause", "Pause preview", "disabled")}
        ${icon("play", "Resume preview", "hidden")}
        ${icon("retake", "Retake", "hidden disabled")}
        ${icon("settings", "Camera settings")}
      </div>
      <div class="ff-camera-settings" hidden>
        <header><span>Camera settings</span>
          ${icon("close", "Close settings", "class=\"icon\"")}
        </header>
        <label for="ff-camera-device">Camera</label>
        <select id="ff-camera-device" aria-label="Camera" disabled></select>
        <label>Preview and photo resolution</label>
        <p class="ff-camera-resolution">Automatic</p>
        <button data-action="retry" type="button">Refresh cameras</button>
      </div>
    </div>`;
  const video = dialog.querySelector("video");
  const photo = dialog.querySelector("img");
  const outline = dialog.querySelector(".ff-camera-outline");
  const view = dialog.querySelector(".ff-camera-view");
  const dock = dialog.querySelector(".ff-camera-dock");
  const settings = dialog.querySelector(".ff-camera-settings");
  const resolution = dialog.querySelector(".ff-camera-resolution");
  const select = dialog.querySelector("select");
  const status = dialog.querySelector("[role=status]");
  const button = action => dialog.querySelector(`[data-action=${action}]`);
  const analysisCanvas = analyze ? document.createElement("canvas") : null;
  const previousFocus = document.activeElement;
  let stream = null;
  let captured = null;
  let suggestedCrop = null;
  let photoUrl = null;
  let mirrored = false;
  let mirrorOverride = null;
  let rotation = 0;
  let cameraCount = 0;
  let cameraIds = [];
  let photoWidth = 0;
  let photoHeight = 0;
  let generation = 0;
  let closed = false;
  let analysisSession = 0;
  let analysisTimer = null;
  let detectedCorners = null;
  let imageRect = null;

  function drawOutline() {
    if (!detectedCorners || !imageRect) {
      outline.replaceChildren();
      return;
    }
    const { x, y, width, height } = imageRect;
    const corners = ["topLeft", "topRight", "bottomRight", "bottomLeft"]
      .map(name => detectedCorners[name])
      .map(point => ({
        x: x + (mirrored ? 1 - point.x : point.x) * width,
        y: y + point.y * height,
      }));
    outline.setAttribute("viewBox",
      `0 0 ${view.clientWidth} ${view.clientHeight}`);
    const polygon = document.createElementNS("http://www.w3.org/2000/svg",
      "polygon");
    polygon.setAttribute("points", corners.map(point =>
      `${point.x},${point.y}`).join(" "));
    const knobs = corners.map(point => {
      const circle = document.createElementNS(
        "http://www.w3.org/2000/svg", "circle");
      circle.setAttribute("cx", point.x);
      circle.setAttribute("cy", point.y);
      circle.setAttribute("r", 5);
      return circle;
    });
    outline.replaceChildren(polygon, ...knobs);
  }

  function resetAnalysis() {
    ++analysisSession;
    clearTimeout(analysisTimer);
    analysisTimer = null;
    detectedCorners = null;
    drawOutline();
    button("capture").style.setProperty("--progress-angle", "0deg");
  }

  function layout() {
    const width = view.clientWidth;
    const height = view.clientHeight;
    if (!width || !height) return;
    const sideways = rotation % 2 !== 0;
    video.style.width = `${sideways ? height : width}px`;
    video.style.height = `${sideways ? width : height}px`;
    video.style.transform = `translate(-50%, -50%) ` +
      `${mirrored ? "scaleX(-1) " : ""}rotate(${rotation * 90}deg)`;
    const sourceWidth = captured ? photoWidth : video.videoWidth;
    const sourceHeight = captured ? photoHeight : video.videoHeight;
    const aspectWidth = sideways && !captured ? sourceHeight : sourceWidth;
    const aspectHeight = sideways && !captured ? sourceWidth : sourceHeight;
    const scale = Math.min(width / (aspectWidth || width),
      height / (aspectHeight || height));
    const imageWidth = (aspectWidth || width) * scale;
    const imageHeight = (aspectHeight || height) * scale;
    const imageX = (width - imageWidth) / 2;
    const imageY = (height - imageHeight) / 2;
    imageRect = {
      x: imageX, y: imageY, width: imageWidth, height: imageHeight,
    };
    drawOutline();
    const right = width - imageX - imageWidth;
    const bottom = height - imageY - imageHeight;
    const vertical = right >= 82 && height >= 322;
    const below = !vertical && bottom >= 82 && width >= 322;
    const compact = !vertical && !below && imageWidth < 322;
    dock.classList.toggle("vertical", vertical);
    dock.classList.toggle("compact", compact);
    const dockWidth = vertical ? 58 : compact ? 154 : 298;
    const dockHeight = vertical ? 298 : compact ? 106 : 58;
    const dockX = vertical ? imageX + imageWidth + 12 :
      Math.max(0, (width - dockWidth) / 2);
    const dockY = vertical ? (height - dockHeight) / 2 : below ?
      imageY + imageHeight + 12 :
      Math.max(0, imageY + imageHeight - 12 - dockHeight);
    dock.style.left = `${dockX}px`;
    dock.style.top = `${dockY}px`;
    const corner = button("switch");
    corner.style.left = `${Math.max(imageX, imageX + imageWidth - 52)}px`;
    corner.style.top = `${imageY + 12}px`;
    const noticeBottom = vertical || below ?
      height - imageY - imageHeight + 12 : height - dockY + 8;
    status.style.bottom = `${Math.max(12, noticeBottom)}px`;
  }

  function controls(mode) {
    const live = mode === "live";
    const review = mode === "review";
    const paused = mode === "paused";
    button("capture").hidden = review;
    button("use").hidden = !review;
    button("pause").hidden = paused || review;
    button("play").hidden = !paused;
    button("retake").hidden = !review;
    button("capture").disabled = !live;
    button("pause").disabled = !live;
    button("play").disabled = !paused;
    button("use").disabled = !review;
    button("retake").disabled = !review;
    button("rotate").disabled = review;
    button("mirror").disabled = review;
    button("settings").disabled = review;
    button("switch").disabled = review || mode === "starting" ||
      cameraCount < 2;
    select.disabled = review || mode === "starting" || cameraCount < 2;
    if (review) settings.hidden = true;
    layout();
  }

  const observer = globalThis.ResizeObserver ?
    new ResizeObserver(layout) : null;

  function stop() {
    stream?.getTracks().forEach(track => track.stop());
    stream = null;
    video.srcObject = null;
  }

  function clearPhoto() {
    if (photoUrl) URL.revokeObjectURL(photoUrl);
    photoUrl = null;
    captured = null;
    suggestedCrop = null;
    photoWidth = 0;
    photoHeight = 0;
    photo.removeAttribute("src");
    photo.hidden = true;
  }

  function mirror(value) {
    mirrored = value;
    photo.classList.toggle("mirrored", value);
    button("mirror").setAttribute("aria-pressed", String(value));
    layout();
  }

  function drawOriented(context, source, width, height, scale = 1) {
    const outputWidth = rotation % 2 ? height : width;
    const outputHeight = rotation % 2 ? width : height;
    context.translate(outputWidth * scale / 2,
      outputHeight * scale / 2);
    context.rotate(rotation * Math.PI / 2);
    context.scale(scale, scale);
    context.drawImage(source, -width / 2, -height / 2);
  }

  function scheduleAnalysis(session, delay = 200) {
    if (!analyze || closed || !stream || captured || document.hidden) return;
    analysisTimer = setTimeout(() => void analyzeFrame(session), delay);
  }

  function beginAnalysis() {
    resetAnalysis();
    scheduleAnalysis(analysisSession, 100);
  }

  async function analyzeFrame(session) {
    analysisTimer = null;
    if (closed || session !== analysisSession || !stream ||
        document.hidden) return;
    if (!video.videoWidth || !video.videoHeight ||
        video.readyState < 2) {
      scheduleAnalysis(session);
      return;
    }
    if (video.videoWidth * video.videoHeight > pixelBudget) {
      status.textContent = "Photo exceeds remaining project image memory.";
      return;
    }
    const stamp = performance.now();
    let bitmap = null;
    try {
      bitmap = await createImageBitmap(video);
      if (closed || session !== analysisSession || !stream) return;
      const width = bitmap.width;
      const height = bitmap.height;
      const longest = Math.max(width, height);
      const scale = Math.min(1, 640 / longest);
      const outputWidth = rotation % 2 ? height : width;
      const outputHeight = rotation % 2 ? width : height;
      analysisCanvas.width = Math.max(1, Math.round(outputWidth * scale));
      analysisCanvas.height = Math.max(1,
        Math.round(outputHeight * scale));
      const context = analysisCanvas.getContext("2d",
        { willReadFrequently: true });
      if (!context) throw new Error("Camera analysis canvas unavailable.");
      drawOriented(context, bitmap, width, height, scale);
      const data = context.getImageData(0, 0,
        analysisCanvas.width, analysisCanvas.height).data;
      const pixels = new Uint8Array(data.buffer, data.byteOffset,
        data.byteLength);
      const advice = JSON.parse(analyze(pixels, analysisCanvas.width,
        analysisCanvas.height, stamp, performance.now() - stamp, session));
      if (closed || session !== analysisSession || !stream) return;
      // WASM has no std::time::Instant; check the full deadline here.
      if (performance.now() - stamp > 1_000) {
        status.textContent = "Auto-framing is catching up; " +
          "capture manually if needed.";
        beginAnalysis();
        return;
      }
      detectedCorners = advice.corners;
      drawOutline();
      const progress = Math.max(0, Math.min(1, advice.progress));
      button("capture").style.setProperty("--progress-angle",
        `${progress * 360}deg`);
      status.textContent = advice.status;
      if (advice.capture) await takePhoto(bitmap, advice.capture);
    } catch (error) {
      if (!closed && session === analysisSession) {
        status.textContent = `Auto-framing failed (${cameraError(error)}); ` +
          "capture manually.";
      }
    } finally {
      bitmap?.close();
      if (session === analysisSession) scheduleAnalysis(session);
    }
  }

  async function listCameras(turn) {
    try {
      const devices = await navigator.mediaDevices.enumerateDevices();
      if (closed || turn !== generation) return;
      const cameras = devices.filter(device => device.kind === "videoinput");
      cameraIds = cameras.map(device => device.deviceId);
      cameraCount = cameras.length;
      const current = stream?.getVideoTracks()[0]?.getSettings().deviceId ||
        select.value;
      select.replaceChildren(...cameras.map((device, index) =>
        new Option(device.label || `Camera ${index + 1}`, device.deviceId)));
      select.value = current || cameras[0]?.deviceId || "";
      select.disabled = !stream || cameras.length < 2;
      button("switch").disabled = !stream || cameraCount < 2;
    } catch {
      if (closed || turn !== generation) return;
      cameraCount = 0;
      cameraIds = [];
      select.disabled = true;
      button("switch").disabled = true;
    }
  }

  async function start(deviceId) {
    const turn = ++generation;
    resetAnalysis();
    stop();
    clearPhoto();
    video.hidden = false;
    controls("starting");
    status.textContent = "Starting camera…";
    const videoOptions = {
      width: { ideal: 1920 },
      height: { ideal: 1080 },
      ...(deviceId ? { deviceId: { exact: deviceId } } :
        { facingMode: { ideal: "environment" } }),
    };
    try {
      const next = await navigator.mediaDevices.getUserMedia({
        audio: false, video: videoOptions,
      });
      if (closed || turn !== generation) {
        next.getTracks().forEach(track => track.stop());
        return;
      }
      stream = next;
      video.srcObject = next;
      await video.play();
      if (closed || turn !== generation) return;
      const track = next.getVideoTracks()[0];
      const front = track.getSettings().facingMode === "user" ||
        /front|selfie/i.test(track.label);
      mirror(mirrorOverride ?? front);
      const mode = track.getSettings();
      const width = mode.width || video.videoWidth;
      const height = mode.height || video.videoHeight;
      resolution.textContent = width && height ?
        `${width}×${height} (automatic)` : "Automatic";
      controls("live");
      status.textContent = "Position the page, then capture.";
      scheduleAnalysis(analysisSession, 100);
      await listCameras(turn);
      layout();
    } catch (error) {
      if (closed || turn !== generation) return;
      stop();
      controls("paused");
      status.textContent = cameraError(error);
    }
  }

  async function takePhoto(source, crop = null) {
    if (!stream || button("capture").disabled) return;
    const turn = generation;
    resetAnalysis();
    button("capture").disabled = true;
    for (const action of ["rotate", "mirror", "pause", "settings",
      "switch"]) button(action).disabled = true;
    try {
      const width = source.videoWidth || source.width;
      const height = source.videoHeight || source.height;
      if (!width || !height) {
        throw new Error("Camera has not delivered an image yet.");
      }
      if (width * height > pixelBudget) {
        throw new Error("Photo exceeds remaining project image memory.");
      }
      const canvas = document.createElement("canvas");
      canvas.width = rotation % 2 ? height : width;
      canvas.height = rotation % 2 ? width : height;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("Canvas capture is unavailable.");
      drawOriented(context, source, width, height);
      const shot = await new Promise(resolve =>
        canvas.toBlob(resolve, "image/png"));
      if (closed || turn !== generation) return;
      if (!shot) throw new Error("Could not capture photo. Retry.");
      captured = shot;
      suggestedCrop = crop;
      detectedCorners = crop;
      photoWidth = canvas.width;
      photoHeight = canvas.height;
      photoUrl = URL.createObjectURL(captured);
      photo.src = photoUrl;
      photo.hidden = false;
      video.hidden = true;
      stop();
      controls("review");
      status.textContent = "Photo ready. Use it or retake.";
    } catch (error) {
      if (closed || turn !== generation) return;
      status.textContent = cameraError(error);
      controls("live");
      beginAnalysis();
    }
  }

  function visibilityChanged() {
    if (captured) return;
    resetAnalysis();
    if (!document.hidden && stream) {
      scheduleAnalysis(analysisSession, 100);
    }
  }

  document.body.append(dialog);
  dialog.showModal();
  observer?.observe(view);
  document.addEventListener("visibilitychange", visibilityChanged);
  button("cancel").focus();
  return new Promise(resolve => {
    function finish(value) {
      if (closed) return;
      closed = true;
      ++generation;
      resetAnalysis();
      stop();
      clearPhoto();
      observer?.disconnect();
      document.removeEventListener("visibilitychange", visibilityChanged);
      dialog.close();
      dialog.remove();
      previousFocus?.focus?.();
      resolve(value);
    }

    button("cancel").onclick = () => finish(null);
    dialog.oncancel = event => {
      event.preventDefault();
      finish(null);
    };
    button("retry").onclick = () => void start(select.value);
    button("settings").onclick = () => {
      settings.hidden = false;
      button("close").focus();
    };
    button("close").onclick = () => {
      settings.hidden = true;
      button("settings").focus();
    };
    view.onclick = event => {
      if (!event.target.closest(".ff-camera-settings, " +
          "[data-action=settings]")) settings.hidden = true;
    };
    button("switch").onclick = () => {
      const index = cameraIds.indexOf(select.value);
      const next = cameraIds[(index + 1) % cameraIds.length];
      if (!next) return;
      mirrorOverride = null;
      void start(next);
    };
    select.onchange = () => {
      mirrorOverride = null;
      void start(select.value);
    };
    button("rotate").onclick = () => {
      rotation = (rotation + 1) % 4;
      button("rotate").title = `Rotate 90° (${rotation * 90}°)`;
      layout();
      beginAnalysis();
    };
    button("mirror").onclick = () => {
      mirrorOverride = !mirrored;
      mirror(mirrorOverride);
      beginAnalysis();
    };
    button("pause").onclick = () => {
      ++generation;
      resetAnalysis();
      stop();
      video.hidden = true;
      controls("paused");
      status.textContent = "Preview paused. Resume when ready.";
    };
    button("play").onclick = () => void start(select.value);
    button("capture").onclick = () => takePhoto(video);
    button("retake").onclick = () => void start(select.value);
    button("use").onclick = async () => {
      if (!captured || button("use").disabled) return;
      const shot = captured;
      const turn = generation;
      button("use").disabled = true;
      try {
        if (shot.size > 100 * 1024 * 1024) {
          throw new Error("Camera photo is larger than 100 MiB.");
        }
        const bytes = new Uint8Array(await shot.arrayBuffer());
        if (closed || turn !== generation) return;
        finish({ bytes, crop: suggestedCrop ?
          JSON.stringify(suggestedCrop) : null });
      } catch (error) {
        if (closed || turn !== generation) return;
        button("use").disabled = false;
        status.textContent = cameraError(error);
      }
    };
    void start();
  });
}
