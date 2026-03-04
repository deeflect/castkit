#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { chromium } from 'playwright';
import Convert from 'ansi-to-html';

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

  const manifest = {
    ...rawManifest,
    fps,
    snapshots: rawManifest.snapshots.map((snapshot) => ({
      ...snapshot,
      html_lines: snapshot.lines.map((line) => converter.toHtml(line || ' '))
    }))
  };
  manifest.branding = await resolveAvatarWithCache(manifest.branding || {}, avatarCacheDir);

  await fs.mkdir(path.dirname(outputPath), { recursive: true });

  const frameRoot = await fs.mkdtemp(path.join(os.tmpdir(), 'castkit-frames-'));
  const browser = await chromium.launch({ headless: true });
  const viewport = speed === 'fast'
    ? { width: 1920, height: 1080 }
    : { width: 2560, height: 1440 };
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
          if ((s.lines[absolute] || '').startsWith('#')) classes.push('scene');
          if ((s.lines[absolute] || '').startsWith('$')) classes.push('cmd');
          if (absolute === s.active_row) classes.push('active');
          return '<div class="' + classes.join(' ') + '">' + (line || '&nbsp;') + '</div>';
        }).join('');
        this.lastIdx = this.idx;
        this.lastTopRow = topRow;
      }

      terminal.style.transform = 'translate3d(0,' + (-topRowFrac * lineHeight).toFixed(2) + 'px,0)';

      const localRow = activeRowF - this.scrollTop;
      if (s.phase === 'typing' && localRow >= 0 && localRow < maxRows) {
        cursor.style.display = 'block';
        const blink = Math.floor(tMs / 280) % 2 === 0 ? 0.90 : 0.12;
        cursor.style.opacity = String(blink);
        cursor.style.transform = 'translate(' + (cursorColF * 10.5) + 'px, ' + ((localRow * lineHeight) + 2) + 'px)';
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

      camera.style.transform = 'translate3d(' + this.cameraX.toFixed(2) + 'px, ' + this.cameraY.toFixed(2) + 'px, 0) scale(' + this.zoom.toFixed(4) + ')';

      const intro = Math.max(0, 1 - (tMs / 280));
      veil.style.opacity = String(intro);
    }
  };

  window.__setManifest = (manifest) => window.__driver.init(manifest);
  window.__renderAt = (tMs) => window.__driver.renderAt(tMs);
  window.__waitBranding = () => window.__brandingReady;
</script>
</body>
</html>
`);

  await page.evaluate((injectedManifest) => {
    window.__setManifest(injectedManifest);
  }, manifest);
  await page.evaluate(() => {
    if (window.__waitBranding) {
      return window.__waitBranding();
    }
    return Promise.resolve();
  });

  const frameCount = Math.max(1, Math.ceil((manifest.duration_ms / 1000) * fps));
  const fastMode = speed === 'fast';
  const frameExt = fastMode ? 'jpg' : 'png';
  for (let i = 0; i < frameCount; i += 1) {
    const tMs = Math.min(manifest.duration_ms, Math.round((i * 1000) / fps));
    await page.evaluate((timeMs) => {
      window.__renderAt(timeMs);
    }, tMs);

    const framePath = path.join(frameRoot, `frame-${String(i + 1).padStart(6, '0')}.${frameExt}`);
    if (fastMode) {
      await page.screenshot({ path: framePath, type: 'jpeg', quality: 92 });
    } else {
      await page.screenshot({ path: framePath, type: 'png' });
    }
  }

  await browser.close();

  const ffmpeg = spawnSync(
    'ffmpeg',
    [
      '-y',
      '-hide_banner',
      '-loglevel',
      'error',
      '-framerate',
      String(fps),
      '-i',
      path.join(frameRoot, `frame-%06d.${frameExt}`),
      '-vf',
      fastMode
        ? 'format=yuv420p'
        : 'scale=1920:1080:flags=lanczos,unsharp=5:5:0.35:5:5:0.0,format=yuv420p',
      '-c:v',
      'libx264',
      '-preset',
      fastMode ? 'veryfast' : 'slow',
      '-crf',
      fastMode ? '20' : '12',
      '-profile:v',
      'high',
      '-level',
      '4.2',
      '-pix_fmt',
      'yuv420p',
      '-movflags',
      '+faststart',
      outputPath
    ],
    { encoding: 'utf8', maxBuffer: 50 * 1024 * 1024 }
  );

  if (ffmpeg.status !== 0) {
    console.error('ffmpeg encode failed');
    console.error(`frames preserved at: ${frameRoot}`);
    console.error(ffmpeg.stderr || '');
    process.exit(ffmpeg.status ?? 1);
  }

  // Cleanup can be slow with thousands of PNGs; do it in background.
  fs.rm(frameRoot, { recursive: true, force: true }).catch(() => {});

  process.stdout.write(JSON.stringify({ ok: true, output: outputPath, fps, frames: frameCount, speed }) + '\n');
}

main().catch((err) => {
  console.error(err?.stack || err?.message || String(err));
  process.exit(1);
});
