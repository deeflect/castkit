#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import { once } from 'node:events';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';
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
      const resolved = path.resolve(screenshotPath);
      try {
        await fs.access(resolved);
        screenshotUrl = pathToFileURL(resolved).href;
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
    width: 1720px;
    height: 970px;
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
  }
  #page {
    position: absolute;
    left: 70px;
    top: 50px;
    width: 1440px;
    height: 900px;
    border-radius: 12px;
    background: linear-gradient(145deg, rgba(244, 248, 255, 0.98), rgba(229, 238, 250, 0.96));
    overflow: hidden;
    box-shadow: 0 20px 38px rgba(0, 0, 0, 0.22);
  }
  #shot {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 120ms linear;
  }
  #placeholder {
    position: absolute;
    inset: 0;
    background: linear-gradient(160deg, #f6fbff, #e8f1fc);
  }
  #focus {
    position: absolute;
    border-radius: 9px;
    border: 2px solid var(--focus);
    background: var(--focus-bg);
    opacity: 0;
    pointer-events: none;
    transition: opacity 80ms linear;
    will-change: transform, width, height;
  }
  #cursor {
    position: absolute;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: rgba(24, 35, 56, 0.94);
    border: 2px solid #f5fbff;
    box-shadow: 0 6px 15px rgba(0,0,0,0.26);
    transform: translate(-9px, -9px);
    will-change: transform;
  }
  #pulse {
    position: absolute;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: 2px solid rgba(111, 199, 255, 0.92);
    transform: translate(-15px, -15px) scale(0.8);
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

  window.__driver = {
    manifest: null,
    idx: 0,
    zoom: 1,
    cameraX: 0,
    cameraY: 0,
    init(manifest) {
      this.manifest = manifest || {};
      this.idx = 0;
      this.zoom = 1;
      this.cameraX = 0;
      this.cameraY = 0;
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
      const nextAction = actions[Math.min(this.idx + 1, Math.max(0, actions.length - 1))] || action;

      const pageEl = document.getElementById('page');
      const shotEl = document.getElementById('shot');
      const focusEl = document.getElementById('focus');
      const cursorEl = document.getElementById('cursor');
      const pulseEl = document.getElementById('pulse');
      const actionEl = document.getElementById('action');
      const cameraEl = document.getElementById('camera');

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
      } else {
        shotEl.style.opacity = '0';
      }

      const actionStart = action ? num(action.t_ms) : 0;
      const nextStart = nextAction ? num(nextAction.t_ms) : actionStart + 240;
      const span = Math.max(1, nextStart - actionStart);
      const alpha = clamp((tMs - actionStart) / span, 0, 1);

      const curX = num(action?.cursor_x || 720);
      const curY = num(action?.cursor_y || 420);
      const nextX = num(nextAction?.cursor_x || curX);
      const nextY = num(nextAction?.cursor_y || curY);
      const cursorX = curX + ((nextX - curX) * alpha);
      const cursorY = curY + ((nextY - curY) * alpha);

      const focusW = num(action?.target_w || 0);
      const focusH = num(action?.target_h || 0);
      const hasFocus = focusW > 0 && focusH > 0;
      const focusX = num(action?.target_x || 0);
      const focusY = num(action?.target_y || 0);
      if (hasFocus) {
        focusEl.style.opacity = '1';
        focusEl.style.transform = 'translate(' + focusX.toFixed(2) + 'px,' + focusY.toFixed(2) + 'px)';
        focusEl.style.width = focusW.toFixed(2) + 'px';
        focusEl.style.height = focusH.toFixed(2) + 'px';
      } else {
        focusEl.style.opacity = '0';
      }

      const noZoom = Boolean(manifest.no_zoom);
      const targetZoom = noZoom ? 1.0 : (hasFocus ? 1.10 : 1.02);
      this.zoom += (targetZoom - this.zoom) * 0.05;
      const targetCameraX = noZoom ? 0 : clamp(-(cursorX - 720) * 0.10, -95, 95);
      const targetCameraY = noZoom ? 0 : clamp(-(cursorY - 450) * 0.10, -80, 80);
      this.cameraX += (targetCameraX - this.cameraX) * 0.06;
      this.cameraY += (targetCameraY - this.cameraY) * 0.06;
      cameraEl.style.transform = 'translate3d(' + this.cameraX.toFixed(2) + 'px,' + this.cameraY.toFixed(2) + 'px,0) scale(' + this.zoom.toFixed(4) + ')';

      const pageRect = pageEl.getBoundingClientRect();
      const cursorPx = pageRect.left + (cursorX / 1440) * pageRect.width;
      const cursorPy = pageRect.top + (cursorY / 900) * pageRect.height;
      cursorEl.style.transform = 'translate(' + cursorPx.toFixed(2) + 'px,' + cursorPy.toFixed(2) + 'px)';

      const actionType = String(action?.action_type || '');
      const pulseActive = actionType === 'click' && (tMs - actionStart) >= 0 && (tMs - actionStart) <= 280;
      const pulseProgress = clamp((tMs - actionStart) / 280, 0, 1);
      const pulseOpacity = pulseActive ? (1 - pulseProgress) : 0;
      pulseEl.style.opacity = pulseOpacity.toFixed(3);
      pulseEl.style.transform = 'translate(' + cursorPx.toFixed(2) + 'px,' + cursorPy.toFixed(2) + 'px) scale(' + (0.8 + pulseProgress * 1.3).toFixed(3) + ')';

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
