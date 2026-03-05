#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { chromium } from 'playwright';
import Convert from 'ansi-to-html';

const MANIFEST_CHUNK_CHARS = 512 * 1024;

function nowMs() {
  return Number(process.hrtime.bigint()) / 1e6;
}

function roundMs(value) {
  return Math.round(value * 100) / 100;
}

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

function getAvatarSource(branding) {
  if (!branding || typeof branding !== 'object') return '';
  const explicitAvatar = typeof branding.avatar_url === 'string' ? branding.avatar_url.trim() : '';
  if (explicitAvatar) return explicitAvatar;
  const rawX = typeof branding.avatar_x === 'string' ? branding.avatar_x.trim().replace(/^@/, '') : '';
  return rawX ? `https://unavatar.io/x/${encodeURIComponent(rawX)}` : '';
}

async function fetchAsDataUrl(url, timeoutMs = 1200) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { signal: controller.signal, redirect: 'follow' });
    if (!response.ok) return null;
    const contentType = (response.headers.get('content-type') || 'image/png').split(';')[0].trim();
    if (!contentType.startsWith('image/')) return null;
    const body = Buffer.from(await response.arrayBuffer()).toString('base64');
    return `data:${contentType};base64,${body}`;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

async function resolveAvatarWithCache(branding, cacheDir) {
  const source = getAvatarSource(branding);
  if (!source || source.startsWith('data:')) return branding;

  if (!cacheDir) {
    const dataUrl = await fetchAsDataUrl(source);
    if (dataUrl) branding.avatar_url = dataUrl;
    return branding;
  }

  const key = createHash('sha1').update(source).digest('hex');
  const cachePath = path.join(cacheDir, `${key}.json`);
  await fs.mkdir(cacheDir, { recursive: true });

  try {
    const stat = await fs.stat(cachePath);
    const ageMs = Date.now() - stat.mtimeMs;
    if (ageMs <= (14 * 24 * 60 * 60 * 1000)) {
      const cached = JSON.parse(await fs.readFile(cachePath, 'utf8'));
      if (typeof cached?.data_url === 'string' && cached.data_url.startsWith('data:image/')) {
        branding.avatar_url = cached.data_url;
        return branding;
      }
    }
  } catch {
    // Cache miss, continue to network fetch.
  }

  const dataUrl = await fetchAsDataUrl(source);
  if (!dataUrl) return branding;
  branding.avatar_url = dataUrl;
  await fs.writeFile(cachePath, JSON.stringify({ data_url: dataUrl }), 'utf8').catch(() => {});
  return branding;
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

function num(value) {
  return Number.isFinite(value) ? value : 0;
}

function shouldCaptureFrame(previousState, nextState, reusedStreak) {
  if (!nextState || typeof nextState !== 'object') return true;
  if (!previousState || typeof previousState !== 'object') return true;

  if (nextState.phase === 'typing') return true;
  if (reusedStreak >= 1) return true;
  if (nextState.idx !== previousState.idx) return true;
  if (nextState.top_row !== previousState.top_row) return true;
  if (nextState.cursor_visible !== previousState.cursor_visible) return true;

  if (Math.abs(num(nextState.camera_x) - num(previousState.camera_x)) >= 0.42) return true;
  if (Math.abs(num(nextState.camera_y) - num(previousState.camera_y)) >= 0.42) return true;
  if (Math.abs(num(nextState.zoom) - num(previousState.zoom)) >= 0.0016) return true;
  if (Math.abs(num(nextState.terminal_y) - num(previousState.terminal_y)) >= 0.40) return true;
  if (Math.abs(num(nextState.scene_opacity) - num(previousState.scene_opacity)) >= 0.05) return true;
  if (Math.abs(num(nextState.intro_opacity) - num(previousState.intro_opacity)) >= 0.05) return true;
  if ((nextState.overlay_id || '') !== (previousState.overlay_id || '')) return true;
  if (Math.abs(num(nextState.overlay_opacity) - num(previousState.overlay_opacity)) >= 0.05) return true;

  if (nextState.cursor_visible) {
    if (Math.abs(num(nextState.cursor_x) - num(previousState.cursor_x)) >= 0.75) return true;
    if (Math.abs(num(nextState.cursor_y) - num(previousState.cursor_y)) >= 0.75) return true;
    if (Math.abs(num(nextState.cursor_opacity) - num(previousState.cursor_opacity)) >= 0.10) return true;
  }

  return false;
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

  const ffmpeg = spawn('ffmpeg', ffmpegArgs, {
    stdio: ['pipe', 'ignore', 'pipe']
  });
  let ffmpegErr = '';
  ffmpeg.stderr.on('data', (chunk) => {
    ffmpegErr += chunk.toString();
  });

  let evaluateMs = 0;
  let screenshotMs = 0;
  let pipeWaitMs = 0;
  let capturedFrames = 0;
  let reusedFrames = 0;
  let lastRenderState = null;
  let lastFrameBuffer = null;
  let reusedStreak = 0;
  const screenshotOpts = pipeInputCodec === 'mjpeg'
    ? { type: 'jpeg', quality: 92 }
    : { type: 'png' };

  for (let i = 0; i < frameCount; i += 1) {
    const tMs = Math.min(manifest.duration_ms, Math.round((i * 1000) / fps));
    const evaluateStartMs = nowMs();
    const renderStateRaw = await page.evaluate((timeMs) => window.__renderAt(timeMs), tMs);
    evaluateMs += nowMs() - evaluateStartMs;
    const renderState = renderStateRaw && typeof renderStateRaw === 'object'
      ? renderStateRaw
      : null;
    const captureFrame = shouldCaptureFrame(lastRenderState, renderState, reusedStreak);

    let frameBuffer = null;
    if (!captureFrame && lastFrameBuffer) {
      reusedFrames += 1;
      reusedStreak += 1;
      frameBuffer = lastFrameBuffer;
    } else {
      const screenshotStartMs = nowMs();
      frameBuffer = await page.screenshot(screenshotOpts);
      screenshotMs += nowMs() - screenshotStartMs;
      capturedFrames += 1;
      reusedStreak = 0;
      lastFrameBuffer = frameBuffer;
    }
    lastRenderState = renderState;

    const pipeStartMs = nowMs();
    if (!ffmpeg.stdin.write(frameBuffer)) {
      await once(ffmpeg.stdin, 'drain');
    }
    pipeWaitMs += nowMs() - pipeStartMs;
  }

  const closeStartMs = nowMs();
  ffmpeg.stdin.end();
  const [code] = await once(ffmpeg, 'close');
  const ffmpegCloseMs = nowMs() - closeStartMs;
  if (code !== 0) {
    throw new Error(`ffmpeg encode failed (${code}): ${ffmpegErr.trim()}`);
  }
  return {
    frameCount,
    videoEncoder: 'libx264',
    captureInputCodec: pipeInputCodec,
    capturedFrames,
    reusedFrames,
    timing: {
      encodeMs: roundMs(nowMs() - encodeStartMs),
      evaluateMs: roundMs(evaluateMs),
      screenshotMs: roundMs(screenshotMs),
      pipeWaitMs: roundMs(pipeWaitMs),
      ffmpegCloseMs: roundMs(ffmpegCloseMs)
    }
  };
}

async function injectManifestInChunks(page, manifest) {
  const manifestJson = JSON.stringify(manifest);
  for (let i = 0; i < manifestJson.length; i += MANIFEST_CHUNK_CHARS) {
    const chunk = manifestJson.slice(i, i + MANIFEST_CHUNK_CHARS);
    await page.evaluate((injectedChunk) => {
      window.__appendManifestChunk(injectedChunk);
    }, chunk);
  }

  await page.evaluate(() => {
    window.__commitManifestChunks();
  });
  return Buffer.byteLength(manifestJson, 'utf8');
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

async function normalizeOverlayEvents(rawEvents) {
  const normalized = [];
  for (const event of asArray(rawEvents)) {
    if (!event || typeof event !== 'object') continue;
    let imageUrl = null;
    const imagePath = typeof event.image_path === 'string' ? event.image_path.trim() : '';
    if (imagePath) {
      const resolved = path.resolve(imagePath);
      try {
        await fs.access(resolved);
        imageUrl = pathToFileURL(resolved).href;
      } catch {
        imageUrl = null;
      }
    }
    normalized.push({
      ...event,
      result_items: asArray(event.result_items),
      image_url: imageUrl
    });
  }
  normalized.sort((a, b) => num(a.t_ms) - num(b.t_ms));
  return normalized;
}

async function main() {
  const args = parseArgs(process.argv);
  if (!args.manifest || !args.output) {
    console.error('usage: node render.mjs --manifest <path> --output <path> [--fps 60]');
    process.exit(2);
  }

  const manifestPath = path.resolve(args.manifest);
  const outputPath = path.resolve(args.output);
  const fps = Math.max(24, Number(args.fps ?? 60));
  const speed = String(args.speed ?? 'quality').toLowerCase() === 'fast' ? 'fast' : 'quality';
  const avatarCacheDir = args['avatar-cache-dir'] ? path.resolve(args['avatar-cache-dir']) : null;

  const rawManifest = JSON.parse(await fs.readFile(manifestPath, 'utf8'));
  const converter = new Convert({ escapeXML: true, newline: false });
  const overlayEvents = await normalizeOverlayEvents(rawManifest.overlay_events);

  const manifest = {
    ...rawManifest,
    fps,
    snapshots: rawManifest.snapshots.map((snapshot) => ({
      t_ms: snapshot.t_ms,
      active_row: snapshot.active_row,
      cursor_col: snapshot.cursor_col,
      phase: snapshot.phase,
      html_lines: snapshot.lines.map((line) => converter.toHtml(line || ' ')),
      line_prefixes: snapshot.lines.map((line) => (line || ' ').charAt(0))
    })),
    overlay_events: overlayEvents
  };
  manifest.branding = await resolveAvatarWithCache(manifest.branding || {}, avatarCacheDir);

  await fs.mkdir(path.dirname(outputPath), { recursive: true });

  const browser = await chromium.launch({ headless: true });
  const viewport = speed === 'fast'
    ? { width: 1920, height: 1080 }
    : { width: 2304, height: 1296 };
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: 1
  });
  const page = await context.newPage();

  await page.setContent(`
<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<style>
  :root {
    --bg0: #0b1220;
    --bg1: #162236;
    --title: #c9d5e7;
    --line: #e9f0fb;
    --line-dim: #95a3bb;
    --line-cmd: #9dd3ff;
    --line-active: rgba(45, 89, 151, 0.30);
    --cursor: rgba(241, 247, 255, 0.92);
    --cursor-glow: rgba(188, 223, 255, 0.65);
    --hud-bg: rgba(8, 18, 34, 0.58);
    --hud-border: rgba(184, 206, 236, 0.22);
  }
  html, body {
    margin: 0;
    width: 100%;
    height: 100%;
    background: radial-gradient(1300px 860px at 18% 13%, #1f2e48 0%, transparent 53%),
                radial-gradient(1180px 780px at 86% 84%, #1a2f45 0%, transparent 47%),
                linear-gradient(165deg, var(--bg0), var(--bg1));
    overflow: hidden;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
    -webkit-font-smoothing: antialiased;
    text-rendering: geometricPrecision;
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
    background: rgba(8, 13, 24, 0.95);
    box-shadow: 0 30px 78px rgba(0, 0, 0, 0.47), 0 12px 26px rgba(0, 0, 0, 0.26);
    border: 1px solid rgba(196, 215, 245, 0.09);
  }
  #titlebar {
    height: 46px;
    background: linear-gradient(180deg, rgba(29,43,67,0.96), rgba(18,31,50,0.98));
    border-bottom: 1px solid rgba(196, 215, 245, 0.13);
    display: flex;
    align-items: center;
    position: relative;
  }
  .lights {
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
  #title {
    position: absolute;
    width: 100%;
    text-align: center;
    color: var(--title);
    font-size: 14px;
    letter-spacing: 0.2px;
  }
  #terminalWrap {
    position: relative;
    width: 100%;
    height: calc(100% - 46px);
    overflow: hidden;
    padding: 20px 28px;
    box-sizing: border-box;
  }
  #terminal {
    position: absolute;
    inset: 20px 28px;
    color: var(--line);
    font-size: 19px;
    line-height: 24px;
    white-space: nowrap;
    will-change: transform;
  }
  .line {
    height: 24px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: pre;
  }
  .line.scene { color: var(--line-dim); }
  .line.cmd { color: var(--line-cmd); }
  .line.active {
    background: var(--line-active);
    border-radius: 5px;
  }
  #cursor {
    position: absolute;
    width: 10px;
    height: 20px;
    background: var(--cursor);
    border-radius: 2px;
    mix-blend-mode: screen;
    box-shadow: 0 0 11px var(--cursor-glow);
    will-change: transform, opacity;
  }
  #overlayLayer {
    position: absolute;
    max-width: 520px;
    min-width: 260px;
    opacity: 0;
    will-change: transform, opacity;
    pointer-events: none;
  }
  .overlay-card {
    border-radius: 14px;
    background: rgba(8, 18, 34, 0.84);
    border: 1px solid rgba(184, 206, 236, 0.28);
    box-shadow: 0 18px 34px rgba(0, 0, 0, 0.35);
    backdrop-filter: blur(6px);
    color: var(--line);
    overflow: hidden;
  }
  .overlay-title {
    font-size: 13px;
    letter-spacing: 0.25px;
    color: var(--line-dim);
    padding: 10px 12px 8px;
    border-bottom: 1px solid rgba(184, 206, 236, 0.16);
  }
  .overlay-image {
    display: block;
    width: 100%;
    max-height: 310px;
    object-fit: contain;
    background: rgba(0, 0, 0, 0.18);
  }
  .overlay-body {
    padding: 10px 12px 12px;
    display: grid;
    gap: 7px;
  }
  .overlay-item {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 14px;
    font-size: 14px;
  }
  .overlay-item-label {
    color: var(--line-dim);
    white-space: nowrap;
  }
  .overlay-item-value {
    color: var(--line);
    text-align: right;
    word-break: break-word;
  }
  #veil {
    position: absolute;
    inset: 0;
    background: #0a1220;
    pointer-events: none;
    opacity: 1;
  }
  #hud {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  #presenter {
    position: absolute;
    top: 28px;
    right: 30px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 11px 7px 7px;
    border-radius: 999px;
    color: var(--line);
    font-size: 13px;
    letter-spacing: 0.2px;
    background: var(--hud-bg);
    border: 1px solid var(--hud-border);
    backdrop-filter: blur(3px);
    opacity: 0.96;
  }
  #presenter.hidden {
    display: none;
  }
  #presenter-avatar {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    object-fit: cover;
    border: 1px solid rgba(255, 255, 255, 0.22);
  }
  #watermark {
    position: absolute;
    right: 24px;
    bottom: 18px;
    font-size: 12px;
    letter-spacing: 0.45px;
    color: rgba(233, 240, 251, 0.55);
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
  }
  #watermark.hidden {
    display: none;
  }
  #sceneTag {
    position: absolute;
    left: 28px;
    bottom: 24px;
    max-width: 56%;
    padding: 8px 12px;
    border-radius: 10px;
    border: 1px solid var(--hud-border);
    background: var(--hud-bg);
    color: var(--line);
    font-size: 15px;
    letter-spacing: 0.2px;
    opacity: 0;
    transform: translateY(8px);
    transition: opacity 80ms linear, transform 80ms linear;
    backdrop-filter: blur(3px);
  }
</style>
</head>
<body>
<div id="stage">
  <div id="camera">
    <div id="window">
      <div id="titlebar">
        <div class="lights"><span class="light l1"></span><span class="light l2"></span><span class="light l3"></span></div>
        <div id="title">castkit • screenstudio mode</div>
      </div>
      <div id="terminalWrap">
        <div id="terminal"></div>
        <div id="cursor"></div>
        <div id="overlayLayer"></div>
      </div>
    </div>
  </div>
  <div id="hud">
    <div id="presenter" class="hidden">
      <img id="presenter-avatar" alt="avatar" />
      <span id="presenter-label"></span>
    </div>
    <div id="sceneTag"></div>
    <div id="watermark" class="hidden"></div>
  </div>
  <div id="veil"></div>
</div>
<script>
  window.__brandingReady = Promise.resolve();

  function colorSupported(value) {
    return typeof value === 'string' && value.trim().length > 0 && CSS.supports('color', value);
  }

  function hexToRgb(value) {
    if (typeof value !== 'string') return null;
    const raw = value.trim().replace('#', '');
    if (raw.length === 3 && /^[0-9a-fA-F]{3}$/.test(raw)) {
      return {
        r: parseInt(raw[0] + raw[0], 16),
        g: parseInt(raw[1] + raw[1], 16),
        b: parseInt(raw[2] + raw[2], 16)
      };
    }
    if (raw.length === 6 && /^[0-9a-fA-F]{6}$/.test(raw)) {
      return {
        r: parseInt(raw.slice(0, 2), 16),
        g: parseInt(raw.slice(2, 4), 16),
        b: parseInt(raw.slice(4, 6), 16)
      };
    }
    return null;
  }

  function applyBranding(branding) {
    if (!branding || typeof branding !== 'object') return;
    const root = document.documentElement;
    const titleEl = document.getElementById('title');
    const presenterEl = document.getElementById('presenter');
    const avatarEl = document.getElementById('presenter-avatar');
    const labelEl = document.getElementById('presenter-label');
    const watermarkEl = document.getElementById('watermark');

    if (typeof branding.title === 'string' && branding.title.trim().length > 0) {
      titleEl.textContent = branding.title.trim();
    }

    const map = [
      ['bg_primary', '--bg0'],
      ['bg_secondary', '--bg1'],
      ['text_primary', '--line'],
      ['text_muted', '--line-dim'],
      ['command_text', '--line-cmd']
    ];

    for (const [key, cssVar] of map) {
      const value = branding[key];
      if (colorSupported(value)) {
        root.style.setProperty(cssVar, value.trim());
      }
    }

    if (colorSupported(branding.accent)) {
      const accent = branding.accent.trim();
      root.style.setProperty('--line-cmd', accent);
      const rgb = hexToRgb(accent);
      if (rgb) {
        root.style.setProperty('--line-active', 'rgba(' + rgb.r + ', ' + rgb.g + ', ' + rgb.b + ', 0.30)');
        root.style.setProperty('--cursor', 'rgba(' + rgb.r + ', ' + rgb.g + ', ' + rgb.b + ', 0.95)');
        root.style.setProperty('--cursor-glow', 'rgba(' + rgb.r + ', ' + rgb.g + ', ' + rgb.b + ', 0.60)');
      }
    }

    if (typeof branding.watermark_text === 'string' && branding.watermark_text.trim().length > 0) {
      watermarkEl.classList.remove('hidden');
      watermarkEl.textContent = branding.watermark_text.trim();
    } else {
      watermarkEl.classList.add('hidden');
      watermarkEl.textContent = '';
    }

    const rawX = typeof branding.avatar_x === 'string' ? branding.avatar_x.trim().replace(/^@/, '') : '';
    const explicitAvatar = typeof branding.avatar_url === 'string' ? branding.avatar_url.trim() : '';
    const avatarSrc = explicitAvatar || (rawX ? ('https://unavatar.io/x/' + encodeURIComponent(rawX)) : '');
    const label = (typeof branding.avatar_label === 'string' && branding.avatar_label.trim().length > 0)
      ? branding.avatar_label.trim()
      : (rawX ? ('@' + rawX) : '');

    if (!avatarSrc) {
      presenterEl.classList.add('hidden');
      avatarEl.removeAttribute('src');
      labelEl.textContent = '';
      window.__brandingReady = Promise.resolve();
      return;
    }

    presenterEl.classList.remove('hidden');
    labelEl.textContent = label;
    window.__brandingReady = new Promise((resolve) => {
      let done = false;
      const finish = () => {
        if (done) return;
        done = true;
        resolve();
      };
      const timeout = setTimeout(finish, 1200);
      avatarEl.onload = () => {
        clearTimeout(timeout);
        finish();
      };
      avatarEl.onerror = () => {
        clearTimeout(timeout);
        presenterEl.classList.add('hidden');
        finish();
      };
      avatarEl.src = avatarSrc;
    });
  }

  function escapeHtml(value) {
    return String(value ?? '')
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;');
  }

  function num(value) {
    return Number.isFinite(value) ? value : 0;
  }

  function overlayPosition(position) {
    switch (position) {
      case 'top_left':
        return { left: 16, top: 16 };
      case 'bottom_left':
        return { left: 16, bottom: 16 };
      case 'bottom_right':
        return { right: 16, bottom: 16 };
      case 'center':
        return { center: true };
      case 'top_right':
      default:
        return { right: 16, top: 16 };
    }
  }

  function overlayMarkup(event) {
    const title = typeof event.title === 'string' && event.title.trim().length > 0
      ? event.title.trim()
      : '';
    const titleHtml = title ? ('<div class="overlay-title">' + escapeHtml(title) + '</div>') : '';

    if (event.artifact_type === 'image' && typeof event.image_url === 'string' && event.image_url.length > 0) {
      return '<div class="overlay-card">' +
        titleHtml +
        '<img class="overlay-image" src="' + escapeHtml(event.image_url) + '" alt="artifact image" />' +
      '</div>';
    }

    const items = Array.isArray(event.result_items) ? event.result_items : [];
    const rows = items.map((item) => (
      '<div class="overlay-item">' +
        '<span class="overlay-item-label">' + escapeHtml(item?.label ?? '') + '</span>' +
        '<span class="overlay-item-value">' + escapeHtml(item?.value ?? '') + '</span>' +
      '</div>'
    )).join('');
    return '<div class="overlay-card">' + titleHtml + '<div class="overlay-body">' + rows + '</div></div>';
  }

  window.__driver = {
    manifest: null,
    idx: 0,
    sceneIdx: -1,
    zoom: 1.0,
    cameraX: 0.0,
    cameraY: 0.0,
    scrollTop: 0.0,
    lastIdx: -1,
    lastTopRow: -1,
    overlayKey: '',
    init(manifest) {
      this.manifest = manifest;
      this.idx = 0;
      this.sceneIdx = -1;
      this.zoom = 1.0;
      this.cameraX = 0.0;
      this.cameraY = 0.0;
      this.scrollTop = 0.0;
      this.lastIdx = -1;
      this.lastTopRow = -1;
      this.overlayKey = '';
      applyBranding(manifest.branding || {});
    },
    renderAt(tMs) {
      const manifest = this.manifest;
      if (!manifest) return;

      const terminal = document.getElementById('terminal');
      const cursor = document.getElementById('cursor');
      const camera = document.getElementById('camera');
      const veil = document.getElementById('veil');
      const sceneTag = document.getElementById('sceneTag');
      const overlayLayer = document.getElementById('overlayLayer');

      const lineHeight = manifest.line_height || 24;
      const maxRows = Math.floor((970 - 46 - 40) / lineHeight);

      while (this.idx + 1 < manifest.snapshots.length && manifest.snapshots[this.idx + 1].t_ms <= tMs) {
        this.idx += 1;
      }

      const s = manifest.snapshots[this.idx];
      const next = manifest.snapshots[Math.min(this.idx + 1, manifest.snapshots.length - 1)];
      const span = Math.max(1, next.t_ms - s.t_ms);
      const alpha = Math.max(0, Math.min(1, (tMs - s.t_ms) / span));
      const activeRowF = s.active_row + ((next.active_row - s.active_row) * alpha);
      const cursorColF = s.cursor_col + ((next.cursor_col - s.cursor_col) * alpha);
      const sceneCues = manifest.scene_cues || [];

      while (this.sceneIdx + 1 < sceneCues.length && sceneCues[this.sceneIdx + 1].t_ms <= tMs) {
        this.sceneIdx += 1;
      }

      if (this.sceneIdx >= 0 && sceneCues[this.sceneIdx]) {
        const cue = sceneCues[this.sceneIdx];
        const elapsed = tMs - cue.t_ms;
        const total = 2200;
        if (elapsed >= 0 && elapsed <= total) {
          let opacity = 1;
          if (elapsed < 180) opacity = elapsed / 180;
          else if (elapsed > 1700) opacity = (total - elapsed) / (total - 1700);
          opacity = Math.max(0, Math.min(1, opacity));
          sceneTag.textContent = cue.title || '';
          sceneTag.style.opacity = opacity.toFixed(3);
          sceneTag.style.transform = 'translateY(' + (8 - (opacity * 8)).toFixed(1) + 'px)';
        } else {
          sceneTag.style.opacity = '0';
          sceneTag.style.transform = 'translateY(8px)';
        }
      } else {
        sceneTag.style.opacity = '0';
        sceneTag.style.transform = 'translateY(8px)';
      }

      const overlayEvents = manifest.overlay_events || [];
      let activeOverlay = null;
      for (let i = overlayEvents.length - 1; i >= 0; i -= 1) {
        const event = overlayEvents[i];
        if (!event || typeof event !== 'object') continue;
        const startMs = num(event.t_ms);
        const endMs = startMs + Math.max(300, num(event.show_ms || 0));
        if (tMs >= startMs && tMs <= (endMs + 180)) {
          activeOverlay = event;
          break;
        }
      }

      let overlayOpacity = 0;
      let overlayId = '';
      if (activeOverlay) {
        const startMs = num(activeOverlay.t_ms);
        const showMs = Math.max(300, num(activeOverlay.show_ms || 0));
        const elapsed = tMs - startMs;
        const enterMs = 180;
        const exitMs = 160;
        const exitStart = showMs - exitMs;

        if (elapsed < 0 || elapsed > (showMs + exitMs)) {
          overlayLayer.style.opacity = '0';
          overlayLayer.style.display = 'none';
        } else {
          if (elapsed < enterMs) overlayOpacity = Math.max(0, Math.min(1, elapsed / enterMs));
          else if (elapsed > exitStart) overlayOpacity = Math.max(0, Math.min(1, (showMs + exitMs - elapsed) / (exitMs * 2)));
          else overlayOpacity = 1;

          overlayId = String(activeOverlay.step_id || '') + ':' + String(activeOverlay.t_ms || 0) + ':' + String(activeOverlay.artifact_type || '');
          if (this.overlayKey !== overlayId) {
            overlayLayer.innerHTML = overlayMarkup(activeOverlay);
            this.overlayKey = overlayId;
          }

          overlayLayer.style.display = 'block';
          overlayLayer.style.opacity = overlayOpacity.toFixed(3);

          overlayLayer.style.left = 'auto';
          overlayLayer.style.right = 'auto';
          overlayLayer.style.top = 'auto';
          overlayLayer.style.bottom = 'auto';

          const pos = overlayPosition(activeOverlay.position);
          if (pos.center) {
            overlayLayer.style.left = '50%';
            overlayLayer.style.top = '50%';
          } else {
            if (typeof pos.left === 'number') overlayLayer.style.left = pos.left + 'px';
            if (typeof pos.right === 'number') overlayLayer.style.right = pos.right + 'px';
            if (typeof pos.top === 'number') overlayLayer.style.top = pos.top + 'px';
            if (typeof pos.bottom === 'number') overlayLayer.style.bottom = pos.bottom + 'px';
          }

          const enterKind = String(activeOverlay.enter || 'fade');
          let offsetX = 0;
          let offsetY = 0;
          if (enterKind === 'slide') {
            const shift = (1 - overlayOpacity) * 26;
            offsetY = shift;
          } else if (enterKind === 'scale') {
            const scale = 0.94 + (overlayOpacity * 0.06);
            if (pos.center) {
              overlayLayer.style.transform = 'translate(-50%, -50%) scale(' + scale.toFixed(4) + ')';
            } else {
              overlayLayer.style.transform = 'translate(0px, 0px) scale(' + scale.toFixed(4) + ')';
            }
          }

          if (enterKind !== 'scale') {
            if (pos.center) {
              overlayLayer.style.transform = 'translate(calc(-50% + ' + offsetX.toFixed(1) + 'px), calc(-50% + ' + offsetY.toFixed(1) + 'px))';
            } else {
              overlayLayer.style.transform = 'translate(' + offsetX.toFixed(1) + 'px, ' + offsetY.toFixed(1) + 'px)';
            }
          }
        }
      } else {
        overlayLayer.style.opacity = '0';
        overlayLayer.style.display = 'none';
        this.overlayKey = '';
      }

      const desiredTop = Math.max(0, activeRowF - (maxRows * 0.62));
      const scrollDelta = (desiredTop - this.scrollTop) * 0.12;
      this.scrollTop += Math.max(-0.65, Math.min(0.65, scrollDelta));
      const topRow = Math.max(0, Math.floor(this.scrollTop));
      const topRowFrac = this.scrollTop - topRow;

      if (this.lastIdx !== this.idx || this.lastTopRow !== topRow) {
        const visible = s.html_lines.slice(topRow, topRow + maxRows + 1);
        terminal.innerHTML = visible.map((line, i) => {
          const absolute = topRow + i;
          const classes = ['line'];
          if ((s.line_prefixes[absolute] || '') === '#') classes.push('scene');
          if ((s.line_prefixes[absolute] || '') === '$') classes.push('cmd');
          if (absolute === s.active_row) classes.push('active');
          return '<div class="' + classes.join(' ') + '">' + (line || '&nbsp;') + '</div>';
        }).join('');
        this.lastIdx = this.idx;
        this.lastTopRow = topRow;
      }

      const terminalTransform = 'translate3d(0,' + (-topRowFrac * lineHeight).toFixed(2) + 'px,0)';
      terminal.style.transform = terminalTransform;

      const localRow = activeRowF - this.scrollTop;
      let cursorVisible = false;
      let cursorOpacity = '0';
      let cursorTransform = '';
      let cursorBlink = 0;
      if (s.phase === 'typing' && localRow >= 0 && localRow < maxRows) {
        cursorVisible = true;
        cursor.style.display = 'block';
        const blink = Math.floor(tMs / 280) % 2 === 0 ? 0.90 : 0.12;
        cursorBlink = blink;
        cursorOpacity = String(blink);
        cursor.style.opacity = cursorOpacity;
        cursorTransform = 'translate(' + (cursorColF * 10.5) + 'px, ' + ((localRow * lineHeight) + 2) + 'px)';
        cursor.style.transform = cursorTransform;
      } else {
        cursor.style.display = 'none';
      }

      const noZoom = Boolean(manifest.no_zoom);
      const cursorInfluence = Math.max(0, Math.min(1, cursorColF / 72));
      const isTyping = s.phase === 'typing';
      let targetZoom = 1.01;
      if (isTyping) targetZoom = 1.08 + (0.17 * cursorInfluence);
      if (noZoom) targetZoom = 1.0;
      const zoomDelta = (targetZoom - this.zoom) * 0.05;
      this.zoom += Math.max(-0.0032, Math.min(0.0032, zoomDelta));

      const cursorPx = isTyping ? (cursorColF * 10.5) : 580;
      let targetCameraX = -(cursorPx - 580) * 0.12;
      targetCameraX = Math.max(-95, Math.min(95, targetCameraX));
      if (noZoom) targetCameraX = 0;
      const cameraXDelta = (targetCameraX - this.cameraX) * 0.055;
      this.cameraX += Math.max(-1.4, Math.min(1.4, cameraXDelta));

      const rowInPixels = localRow * lineHeight;
      const yStrength = isTyping ? -0.165 : -0.09;
      const targetCameraY = (rowInPixels - 405) * yStrength;
      const targetCameraYFinal = noZoom ? 0 : targetCameraY;
      const cameraYDelta = (targetCameraYFinal - this.cameraY) * 0.06;
      this.cameraY += Math.max(-1.9, Math.min(1.9, cameraYDelta));

      const cameraTransform = 'translate3d(' + this.cameraX.toFixed(2) + 'px, ' + this.cameraY.toFixed(2) + 'px, 0) scale(' + this.zoom.toFixed(4) + ')';
      camera.style.transform = cameraTransform;

      const intro = Math.max(0, 1 - (tMs / 280));
      const introOpacity = intro.toFixed(3);
      veil.style.opacity = introOpacity;

      return {
        idx: this.idx,
        phase: s.phase || 'idle',
        top_row: topRow,
        terminal_y: -topRowFrac * lineHeight,
        cursor_visible: cursorVisible,
        cursor_x: cursorVisible ? (cursorColF * 10.5) : 0,
        cursor_y: cursorVisible ? ((localRow * lineHeight) + 2) : 0,
        cursor_opacity: cursorVisible ? cursorBlink : 0,
        camera_x: this.cameraX,
        camera_y: this.cameraY,
        zoom: this.zoom,
        intro_opacity: Number(introOpacity),
        scene_opacity: Number(sceneTag.style.opacity || 0),
        overlay_id: overlayId,
        overlay_opacity: overlayOpacity
      };
    }
  };

  window.__setManifest = (manifest) => window.__driver.init(manifest);
  window.__manifestJsonBuffer = '';
  window.__appendManifestChunk = (chunk) => {
    window.__manifestJsonBuffer += chunk;
  };
  window.__commitManifestChunks = () => {
    const manifest = JSON.parse(window.__manifestJsonBuffer || '{}');
    window.__manifestJsonBuffer = '';
    window.__setManifest(manifest);
  };
  window.__renderAt = (tMs) => window.__driver.renderAt(tMs);
  window.__waitBranding = () => window.__brandingReady;
</script>
</body>
</html>
`);

  const manifestBytes = await injectManifestInChunks(page, manifest);
  await page.evaluate(() => {
    if (window.__waitBranding) {
      return window.__waitBranding();
    }
    return Promise.resolve();
  });

  let frameCount = 0;
  let videoEncoder = 'libx264';
  let captureInputCodec = 'png';
  let capturedFrames = 0;
  let reusedFrames = 0;
  let encodeTiming = {
    encodeMs: 0,
    evaluateMs: 0,
    screenshotMs: 0,
    pipeWaitMs: 0,
    ffmpegCloseMs: 0
  };
  const totalStartMs = nowMs();
  try {
    const encodeResult = await encodeFramesViaPipe({
      page,
      manifest,
      fps,
      speed,
      outputPath
    });
    frameCount = encodeResult.frameCount;
    videoEncoder = encodeResult.videoEncoder;
    captureInputCodec = encodeResult.captureInputCodec;
    capturedFrames = encodeResult.capturedFrames;
    reusedFrames = encodeResult.reusedFrames;
    encodeTiming = encodeResult.timing;
  } finally {
    await browser.close();
  }

  process.stdout.write(JSON.stringify({
    ok: true,
    output: outputPath,
    fps,
    manifest_bytes: manifestBytes,
    frames: frameCount,
    speed,
    video_encoder: videoEncoder,
    capture: {
      viewport,
      input_codec: captureInputCodec,
      captured_frames: capturedFrames,
      reused_frames: reusedFrames
    },
    timing_ms: {
      totalMs: roundMs(nowMs() - totalStartMs),
      ...encodeTiming
    }
  }) + '\n');
}

main().catch((err) => {
  console.error(err?.stack || err?.message || String(err));
  process.exit(1);
});
