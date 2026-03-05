#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import { once } from 'node:events';
import { spawn } from 'node:child_process';
import { chromium } from 'playwright';

function parseArgs(argv) {
  const args = {};
  for (let i = 2; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith('--')) continue;
    const key = token.slice(2);
    const value = argv[i + 1];
    if (!value || value.startsWith('--')) {
      args[key] = true;
      continue;
    }
    args[key] = value;
    i += 1;
  }
  return args;
}

function nowMs() {
  return Number(process.hrtime.bigint()) / 1e6;
}

function roundMs(value) {
  return Math.round(value * 100) / 100;
}

function num(value) {
  return Number.isFinite(value) ? value : 0;
}

function mimeForPath(filePath) {
  const ext = path.extname(String(filePath || '')).toLowerCase();
  switch (ext) {
    case '.jpg':
    case '.jpeg':
      return 'image/jpeg';
    case '.webp':
      return 'image/webp';
    case '.gif':
      return 'image/gif';
    case '.png':
    default:
      return 'image/png';
  }
}

async function filePathToDataUrl(filePath) {
  const abs = path.resolve(filePath);
  const bytes = await fs.readFile(abs);
  const mime = mimeForPath(abs);
  return `data:${mime};base64,${bytes.toString('base64')}`;
}

function softwareEncodeArgs({ fastMode, balancedQuality }) {
  const vf = fastMode
    ? 'format=yuv420p'
    : balancedQuality
      ? 'scale=1920:1080:flags=bicubic,format=yuv420p'
      : 'scale=1920:1080:flags=lanczos,unsharp=5:5:0.35:5:5:0.0,format=yuv420p';
  return [
    '-vf',
    vf,
    '-c:v',
    'libx264',
    '-preset',
    fastMode ? 'veryfast' : balancedQuality ? 'medium' : 'slow',
    '-crf',
    fastMode ? '20' : balancedQuality ? '16' : '12',
    '-profile:v',
    'high',
    '-level',
    '4.2',
    '-pix_fmt',
    'yuv420p'
  ];
}

function shouldCaptureFrame(previousState, nextState) {
  if (!nextState || typeof nextState !== 'object') return true;
  if (!previousState || typeof previousState !== 'object') return true;
  if (nextState.idx !== previousState.idx) return true;
  if (Math.abs(num(nextState.cursor_x) - num(previousState.cursor_x)) >= 0.7) return true;
  if (Math.abs(num(nextState.cursor_y) - num(previousState.cursor_y)) >= 0.7) return true;
  if (Math.abs(num(nextState.camera_x) - num(previousState.camera_x)) >= 0.4) return true;
  if (Math.abs(num(nextState.camera_y) - num(previousState.camera_y)) >= 0.4) return true;
  if (Math.abs(num(nextState.zoom) - num(previousState.zoom)) >= 0.0012) return true;
  if (Math.abs(num(nextState.pulse) - num(previousState.pulse)) >= 0.05) return true;
  if ((nextState.action_id || '') !== (previousState.action_id || '')) return true;
  return false;
}

async function normalizeActions(rawActions) {
  const actions = Array.isArray(rawActions) ? rawActions : [];
  const normalized = [];
  for (const action of actions) {
    if (!action || typeof action !== 'object') continue;
    let screenshotUrl = null;
    const screenshotPath = typeof action.screenshot_path === 'string' ? action.screenshot_path.trim() : '';
    if (screenshotPath) {
      try {
        screenshotUrl = await filePathToDataUrl(screenshotPath);
      } catch {
        screenshotUrl = null;
      }
    }
    normalized.push({
      ...action,
      screenshot_url: screenshotUrl
    });
  }
  normalized.sort((a, b) => num(a.t_ms) - num(b.t_ms));
  return normalized;
}

async function encodeFramesViaPipe({ page, manifest, fps, speed, outputPath }) {
  const encodeStartMs = nowMs();
  const fastMode = speed === 'fast';
  const balancedQuality = !fastMode && fps <= 45;
  const frameCount = Math.max(1, Math.ceil((manifest.duration_ms / 1000) * fps));
  const pipeInputCodec = fastMode ? 'mjpeg' : 'png';
  const codecArgs = softwareEncodeArgs({ fastMode, balancedQuality });
  const ffmpegArgs = [
    '-y',
    '-hide_banner',
    '-loglevel',
    'error',
    '-f',
    'image2pipe',
    '-vcodec',
    pipeInputCodec,
    '-framerate',
    String(fps),
    '-i',
    '-',
    ...codecArgs,
    '-movflags',
    '+faststart',
    outputPath
  ];
  const ffmpeg = spawn('ffmpeg', ffmpegArgs, { stdio: ['pipe', 'ignore', 'pipe'] });
  let ffmpegErr = '';
  ffmpeg.stderr.on('data', (chunk) => {
    ffmpegErr += chunk.toString();
  });

  let evaluateMs = 0;
  let screenshotMs = 0;
  let pipeWaitMs = 0;
  let capturedFrames = 0;
  let reusedFrames = 0;
  let previousState = null;
  let previousBuffer = null;
  const screenshotOpts = pipeInputCodec === 'mjpeg'
    ? { type: 'jpeg', quality: 92 }
    : { type: 'png' };

  for (let i = 0; i < frameCount; i += 1) {
    const tMs = Math.min(manifest.duration_ms, Math.round((i * 1000) / fps));
    const evaluateStart = nowMs();
    const state = await page.evaluate((timeMs) => window.__renderAt(timeMs), tMs);
    evaluateMs += nowMs() - evaluateStart;

    let frameBuffer = null;
    if (!shouldCaptureFrame(previousState, state) && previousBuffer) {
      frameBuffer = previousBuffer;
      reusedFrames += 1;
    } else {
      const shotStart = nowMs();
      frameBuffer = await page.screenshot(screenshotOpts);
      screenshotMs += nowMs() - shotStart;
      capturedFrames += 1;
      previousBuffer = frameBuffer;
    }
    previousState = state;

    const pipeStart = nowMs();
    if (!ffmpeg.stdin.write(frameBuffer)) {
      await once(ffmpeg.stdin, 'drain');
    }
    pipeWaitMs += nowMs() - pipeStart;
  }

  ffmpeg.stdin.end();
  const [code] = await once(ffmpeg, 'close');
  if (code !== 0) {
    throw new Error(`ffmpeg encode failed (${code}): ${ffmpegErr.trim()}`);
  }

  return {
    frameCount,
    capturedFrames,
    reusedFrames,
    timing: {
      encodeMs: roundMs(nowMs() - encodeStartMs),
      evaluateMs: roundMs(evaluateMs),
      screenshotMs: roundMs(screenshotMs),
      pipeWaitMs: roundMs(pipeWaitMs)
    }
  };
}

async function main() {
  const args = parseArgs(process.argv);
  if (!args.manifest || !args.output) {
    console.error('usage: node render-web.mjs --manifest <path> --output <path> [--fps 60]');
    process.exit(2);
  }

  const manifestPath = path.resolve(args.manifest);
  const outputPath = path.resolve(args.output);
  const fps = Math.max(24, Number(args.fps ?? 60));
  const speed = String(args.speed ?? 'quality').toLowerCase() === 'fast' ? 'fast' : 'quality';
  const rawManifest = JSON.parse(await fs.readFile(manifestPath, 'utf8'));
  const actions = await normalizeActions(rawManifest.actions);
  const manifest = {
    ...rawManifest,
    fps,
    actions
  };

  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const viewport = speed === 'fast'
    ? { width: 1920, height: 1080 }
    : { width: 2304, height: 1296 };
  const context = await browser.newContext({ viewport, deviceScaleFactor: 1 });
  const page = await context.newPage();
  const pageErrors = [];
  page.on('pageerror', (err) => {
    pageErrors.push(err?.stack || err?.message || String(err));
  });

  await page.setContent(`
<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<style>
  :root {
    --bg0: #0a1322;
    --bg1: #15263d;
    --line: #e8f1ff;
    --muted: #9cb2d1;
    --focus: rgba(123, 203, 255, 0.92);
    --focus-bg: rgba(95, 173, 228, 0.22);
  }
  html, body {
    margin: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: radial-gradient(1200px 800px at 20% 15%, #1f3150 0%, transparent 54%),
      radial-gradient(1000px 700px at 80% 78%, #1a3147 0%, transparent 50%),
      linear-gradient(165deg, var(--bg0), var(--bg1));
    font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    color: var(--line);
  }
  #stage {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  #camera {
    width: 1860px;
    height: 1040px;
    transform-origin: center center;
    will-change: transform;
  }
  #window {
    width: 100%;
    height: 100%;
    border-radius: 18px;
    overflow: hidden;
    background: rgba(7, 12, 22, 0.95);
    border: 1px solid rgba(195, 216, 246, 0.14);
    box-shadow: 0 30px 78px rgba(0, 0, 0, 0.47), 0 12px 26px rgba(0, 0, 0, 0.24);
  }
  #titlebar {
    height: 46px;
    background: linear-gradient(180deg, rgba(30, 47, 74, 0.96), rgba(19, 30, 49, 0.98));
    border-bottom: 1px solid rgba(196, 215, 245, 0.13);
    display: flex;
    align-items: center;
    position: relative;
    font-size: 14px;
  }
  #title {
    position: absolute;
    width: 100%;
    text-align: center;
    color: var(--line);
    letter-spacing: 0.2px;
  }
  #lights {
    display: flex;
    gap: 10px;
    padding-left: 16px;
  }
  .light {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    box-shadow: inset 0 1px 1px rgba(255,255,255,0.18), inset 0 -1px 1px rgba(0,0,0,0.3);
  }
  .l1 { background: #ff5f57; }
  .l2 { background: #fdbc2e; }
  .l3 { background: #28c840; }
  #viewport {
    position: relative;
    width: 100%;
    height: calc(100% - 46px);
    overflow: hidden;
    background: rgba(10, 16, 28, 0.92);
  }
  #browserHud {
    position: absolute;
    top: 10px;
    left: 16px;
    right: 16px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 10px;
    pointer-events: none;
  }
  #tabTitle {
    max-width: 24%;
    font-size: 12px;
    color: rgba(226, 236, 248, 0.84);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  #urlPill {
    flex: 1;
    min-width: 0;
    height: 28px;
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 999px;
    padding: 0 12px;
    background: rgba(11, 21, 36, 0.78);
    border: 1px solid rgba(156, 186, 223, 0.28);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
  }
  #urlDot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(98, 224, 172, 0.92);
    box-shadow: 0 0 8px rgba(98, 224, 172, 0.4);
    flex: 0 0 auto;
  }
  #urlText {
    min-width: 0;
    font-size: 12px;
    color: rgba(226, 236, 248, 0.88);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  #page {
    position: absolute;
    left: 0;
    top: 0;
    right: 0;
    bottom: 0;
    border-radius: 0;
    background: rgba(13, 24, 40, 0.96);
    overflow: hidden;
    border: 0;
    box-shadow: none;
  }
  #shot {
    position: absolute;
    left: 0;
    top: 0;
    background: rgba(8, 14, 24, 0.82);
    opacity: 0;
    transition: opacity 120ms linear;
    will-change: transform, width, height;
  }
  #placeholder {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(900px 560px at 14% 18%, rgba(66, 108, 162, 0.26), transparent 56%),
      radial-gradient(900px 560px at 82% 74%, rgba(58, 97, 150, 0.20), transparent 53%),
      linear-gradient(160deg, rgba(18, 30, 48, 0.94), rgba(11, 20, 34, 0.92));
    opacity: 1;
    transition: opacity 120ms linear;
  }
  #focus {
    position: absolute;
    border-radius: 9px;
    border: 2px solid var(--focus);
    background: var(--focus-bg);
    box-shadow: 0 0 24px rgba(123, 203, 255, 0.28), inset 0 0 0 1px rgba(214, 242, 255, 0.35);
    opacity: 0;
    pointer-events: none;
    transition: opacity 80ms linear;
    will-change: transform, width, height;
  }
  #cursor {
    position: absolute;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: rgba(246, 252, 255, 0.96);
    border: 3px solid rgba(20, 33, 54, 0.92);
    box-shadow: 0 8px 18px rgba(0,0,0,0.34), 0 0 10px rgba(219, 242, 255, 0.35);
    transform: translate(-11px, -11px);
    will-change: transform;
  }
  #pulse {
    position: absolute;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    border: 3px solid rgba(114, 214, 255, 0.98);
    background: rgba(110, 207, 255, 0.16);
    box-shadow: 0 0 26px rgba(123, 212, 255, 0.55);
    transform: translate(-23px, -23px) scale(0.70);
    opacity: 0;
    pointer-events: none;
  }
  #action {
    position: absolute;
    left: 28px;
    bottom: 24px;
    max-width: 65%;
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid rgba(184, 206, 236, 0.22);
    background: rgba(8, 18, 34, 0.58);
    color: var(--line);
    font-size: 15px;
    letter-spacing: 0.2px;
    backdrop-filter: blur(3px);
  }
  #watermark {
    position: absolute;
    right: 24px;
    bottom: 18px;
    font-size: 12px;
    letter-spacing: 0.45px;
    color: rgba(233, 240, 251, 0.55);
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
    opacity: 0;
  }
</style>
</head>
<body>
<div id="stage">
  <div id="camera">
    <div id="window">
      <div id="titlebar">
        <div id="lights"><span class="light l1"></span><span class="light l2"></span><span class="light l3"></span></div>
        <div id="title">castkit • web mode</div>
      </div>
      <div id="viewport">
        <div id="browserHud">
          <div id="tabTitle"></div>
          <div id="urlPill">
            <span id="urlDot"></span>
            <span id="urlText"></span>
          </div>
        </div>
        <div id="page">
          <div id="placeholder"></div>
          <img id="shot" alt="screenshot"/>
          <div id="focus"></div>
        </div>
        <div id="cursor"></div>
        <div id="pulse"></div>
      </div>
    </div>
  </div>
  <div id="action"></div>
  <div id="watermark"></div>
</div>
<script>
  function num(value) {
    return Number.isFinite(value) ? value : 0;
  }

  function clamp(value, min, max) {
    return Math.max(min, Math.min(max, value));
  }

  function niceUrl(value) {
    if (typeof value !== 'string' || !value.trim()) return '';
    try {
      const u = new URL(value);
      const path = (u.pathname || '/').replace(/\\/+$/, '') || '/';
      return u.host + path + (u.search || '');
    } catch {
      return value;
    }
  }

  window.__driver = {
    manifest: null,
    idx: 0,
    zoom: 1,
    cameraX: 0,
    cameraY: 0,
    cursorX: 720,
    cursorY: 420,
    cursorReady: false,
    init(manifest) {
      this.manifest = manifest || {};
      this.idx = 0;
      this.zoom = 1;
      this.cameraX = 0;
      this.cameraY = 0;
      this.cursorX = 720;
      this.cursorY = 420;
      this.cursorReady = false;
      const branding = manifest.branding || {};
      if (typeof branding.title === 'string' && branding.title.trim()) {
        document.getElementById('title').textContent = branding.title.trim();
      }
      const watermark = typeof branding.watermark_text === 'string' ? branding.watermark_text.trim() : '';
      const watermarkEl = document.getElementById('watermark');
      if (watermark) {
        watermarkEl.textContent = watermark;
        watermarkEl.style.opacity = '1';
      } else {
        watermarkEl.style.opacity = '0';
      }
    },
    renderAt(tMs) {
      const manifest = this.manifest;
      const actions = manifest.actions || [];
      while (this.idx + 1 < actions.length && num(actions[this.idx + 1].t_ms) <= tMs) {
        this.idx += 1;
      }
      const action = actions[Math.min(this.idx, Math.max(0, actions.length - 1))] || null;

      const pageEl = document.getElementById('page');
      const shotEl = document.getElementById('shot');
      const placeholderEl = document.getElementById('placeholder');
      const focusEl = document.getElementById('focus');
      const cursorEl = document.getElementById('cursor');
      const pulseEl = document.getElementById('pulse');
      const actionEl = document.getElementById('action');
      const cameraEl = document.getElementById('camera');
      const urlTextEl = document.getElementById('urlText');
      const tabTitleEl = document.getElementById('tabTitle');

      let activeShot = null;
      for (let i = this.idx; i >= 0; i -= 1) {
        if (actions[i] && typeof actions[i].screenshot_url === 'string' && actions[i].screenshot_url.length > 0) {
          activeShot = actions[i].screenshot_url;
          break;
        }
      }
      if (activeShot) {
        if (shotEl.src !== activeShot) shotEl.src = activeShot;
        shotEl.style.opacity = '1';
        placeholderEl.style.opacity = '0.05';
      } else {
        shotEl.style.opacity = '0';
        placeholderEl.style.opacity = '1';
      }

      let currentUrl = '';
      let currentTitle = '';
      for (let i = this.idx; i >= 0; i -= 1) {
        const item = actions[i];
        if (!item || typeof item !== 'object') continue;
        if (!currentUrl && typeof item.page_url === 'string' && item.page_url.trim()) {
          currentUrl = item.page_url.trim();
        }
        if (!currentTitle && typeof item.page_title === 'string' && item.page_title.trim()) {
          currentTitle = item.page_title.trim();
        }
        if (currentUrl && currentTitle) break;
      }
      urlTextEl.textContent = niceUrl(currentUrl);
      tabTitleEl.textContent = currentTitle || niceUrl(currentUrl);

      const actionStart = action ? num(action.t_ms) : 0;
      const actionType = String(action?.action_type || '');

      const targetCursorX = num(action?.cursor_x || 720);
      const targetCursorY = num(action?.cursor_y || 420);
      this.cursorX = targetCursorX;
      this.cursorY = targetCursorY;
      this.cursorReady = true;
      const cursorX = this.cursorX;
      const cursorY = this.cursorY;

      const rawFocusW = num(action?.target_w || 0);
      const rawFocusH = num(action?.target_h || 0);
      const rawFocusX = num(action?.target_x || 0);
      const rawFocusY = num(action?.target_y || 0);
      const hasRawFocus = rawFocusW > 0 && rawFocusH > 0;

      const sourceW = Math.max(1, num(shotEl.naturalWidth || 1440));
      const sourceH = Math.max(1, num(shotEl.naturalHeight || 900));
      const pageRect = pageEl.getBoundingClientRect();
      const fitScale = Math.max(pageRect.width / sourceW, pageRect.height / sourceH);
      const drawW = sourceW * fitScale;
      const drawH = sourceH * fitScale;
      const drawOffsetX = (pageRect.width - drawW) * 0.5;
      const drawOffsetY = (pageRect.height - drawH) * 0.5;
      shotEl.style.left = drawOffsetX.toFixed(2) + 'px';
      shotEl.style.top = drawOffsetY.toFixed(2) + 'px';
      shotEl.style.width = drawW.toFixed(2) + 'px';
      shotEl.style.height = drawH.toFixed(2) + 'px';
      const mapPoint = (x, y) => ({
        x: pageRect.left + drawOffsetX + (num(x) / sourceW) * drawW,
        y: pageRect.top + drawOffsetY + (num(y) / sourceH) * drawH
      });

      let mappedCursor = mapPoint(cursorX, cursorY);
      if (hasRawFocus && (actionType === 'click' || actionType === 'type')) {
        mappedCursor = mapPoint(rawFocusX + (rawFocusW * 0.5), rawFocusY + (rawFocusH * 0.5));
      }
      const cursorPx = mappedCursor.x;
      const cursorPy = mappedCursor.y;
      const cursorPageX = cursorPx - pageRect.left;
      const cursorPageY = cursorPy - pageRect.top;

      const focusAgeMs = tMs - actionStart;
      const wantsFocus = actionType === 'click' || actionType === 'type';
      const focusWindowMs = actionType === 'type' ? 1500 : 820;
      const focusVisible = wantsFocus && focusAgeMs >= -20 && focusAgeMs <= focusWindowMs;
      if (focusVisible) {
        let boxX;
        let boxY;
        let boxW;
        let boxH;

        if (actionType === 'type') {
          // Keep typing emphasis stable around the cursor instead of large DOM boxes.
          boxW = clamp(pageRect.width * 0.32, 260, 520);
          boxH = 58;
          boxX = clamp(cursorPageX - (boxW * 0.5), 8, pageRect.width - boxW - 8);
          boxY = clamp(cursorPageY - (boxH * 0.5), 8, pageRect.height - boxH - 8);
        } else {
          let useMappedBox = false;
          if (rawFocusW > 0 && rawFocusH > 0) {
            const topLeft = mapPoint(rawFocusX, rawFocusY);
            const bottomRight = mapPoint(rawFocusX + rawFocusW, rawFocusY + rawFocusH);
            const mappedW = Math.max(8, bottomRight.x - topLeft.x);
            const mappedH = Math.max(8, bottomRight.y - topLeft.y);
            const overlyLarge = mappedW > pageRect.width * 0.45 || mappedH > pageRect.height * 0.22;
            if (!overlyLarge) {
              useMappedBox = true;
              boxX = clamp(topLeft.x - pageRect.left, 8, pageRect.width - mappedW - 8);
              boxY = clamp(topLeft.y - pageRect.top, 8, pageRect.height - mappedH - 8);
              boxW = mappedW;
              boxH = mappedH;
            }
          }
          if (!useMappedBox) {
            boxW = 132;
            boxH = 54;
            boxX = clamp(cursorPageX - (boxW * 0.5), 8, pageRect.width - boxW - 8);
            boxY = clamp(cursorPageY - (boxH * 0.5), 8, pageRect.height - boxH - 8);
          }
        }

        focusEl.style.opacity = '1';
        focusEl.style.transform = 'translate(' + boxX.toFixed(2) + 'px,' + boxY.toFixed(2) + 'px)';
        focusEl.style.width = boxW.toFixed(2) + 'px';
        focusEl.style.height = boxH.toFixed(2) + 'px';
      } else {
        focusEl.style.opacity = '0';
      }

      const noZoom = Boolean(manifest.no_zoom);
      const targetZoom = noZoom ? 1.0 : (focusVisible ? 1.18 : 1.06);
      this.zoom += (targetZoom - this.zoom) * 0.05;
      const targetCameraX = noZoom ? 0 : clamp(-(cursorX - (sourceW * 0.5)) * 0.25, -220, 220);
      const targetCameraY = noZoom ? 0 : clamp(-(cursorY - (sourceH * 0.5)) * 0.22, -170, 170);
      this.cameraX += (targetCameraX - this.cameraX) * 0.06;
      this.cameraY += (targetCameraY - this.cameraY) * 0.06;
      cameraEl.style.transform = 'translate3d(' + this.cameraX.toFixed(2) + 'px,' + this.cameraY.toFixed(2) + 'px,0) scale(' + this.zoom.toFixed(4) + ')';
      cursorEl.style.transform = 'translate(' + (cursorPageX - 11).toFixed(2) + 'px,' + (cursorPageY - 11).toFixed(2) + 'px)';

      const pulseTypes = actionType === 'click';
      const pulseWindowMs = 620;
      const pulseActive = pulseTypes && (tMs - actionStart) >= 0 && (tMs - actionStart) <= pulseWindowMs;
      const pulseProgress = clamp((tMs - actionStart) / pulseWindowMs, 0, 1);
      const pulseOpacity = pulseActive ? (1 - pulseProgress) : 0;
      pulseEl.style.opacity = pulseOpacity.toFixed(3);
      pulseEl.style.transform = 'translate(' + (cursorPageX - 23).toFixed(2) + 'px,' + (cursorPageY - 23).toFixed(2) + 'px) scale(' + (0.70 + pulseProgress * 2.15).toFixed(3) + ')';

      if (action) {
        const label = String(action.action_type || '') + ' • ' + String(action.id || '');
        actionEl.textContent = label;
      } else {
        actionEl.textContent = '';
      }

      return {
        idx: this.idx,
        action_id: action ? String(action.id || '') : '',
        cursor_x: cursorX,
        cursor_y: cursorY,
        camera_x: this.cameraX,
        camera_y: this.cameraY,
        zoom: this.zoom,
        pulse: pulseOpacity
      };
    }
  };

  window.__setManifest = (manifest) => window.__driver.init(manifest);
  window.__renderAt = (tMs) => window.__driver.renderAt(tMs);
</script>
</body>
</html>
`);

  const setManifestType = await page.evaluate(() => typeof window.__setManifest);
  if (setManifestType !== 'function') {
    throw new Error(`web runtime init failed: window.__setManifest=${setManifestType}; pageErrors=${pageErrors.join(' | ')}`);
  }
  await page.evaluate((m) => window.__setManifest(m), manifest);

  let frameCount = 0;
  let timing = { encodeMs: 0, evaluateMs: 0, screenshotMs: 0, pipeWaitMs: 0 };
  const totalStartMs = nowMs();
  try {
    const result = await encodeFramesViaPipe({ page, manifest, fps, speed, outputPath });
    frameCount = result.frameCount;
    timing = result.timing;
  } finally {
    await browser.close();
  }

  process.stdout.write(JSON.stringify({
    ok: true,
    output: outputPath,
    fps,
    frames: frameCount,
    speed,
    timing_ms: {
      totalMs: roundMs(nowMs() - totalStartMs),
      ...timing
    }
  }) + '\n');
}

main().catch((err) => {
  console.error(err?.stack || err?.message || String(err));
  process.exit(1);
});
