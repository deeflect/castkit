#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
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

function asString(value) {
  return typeof value === 'string' ? value : '';
}

function safeFilePart(value) {
  const raw = asString(value).trim().toLowerCase();
  return raw.replace(/[^a-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 64) || 'step';
}

async function captureActionScreenshot(page, cwd, actionId, index) {
  const dir = path.resolve(cwd, '.castkit', 'web-capture');
  await fs.mkdir(dir, { recursive: true });
  const file = `${String(index).padStart(3, '0')}-${safeFilePart(actionId)}.png`;
  const outputPath = path.join(dir, file);
  await page.screenshot({ path: outputPath, fullPage: false });
  return outputPath;
}

function toUrl(rawUrl, baseUrl) {
  const input = asString(rawUrl).trim();
  if (!input) return '';
  if (/^https?:\/\//i.test(input)) return input;
  if (!baseUrl) return input;
  return new URL(input, baseUrl).toString();
}

async function maybeBoundingBox(page, selector) {
  if (!selector) return null;
  try {
    const locator = page.locator(selector).first();
    const bbox = await locator.boundingBox();
    return bbox || null;
  } catch {
    return null;
  }
}

async function moveMouseToTarget(page, selector) {
  if (!selector) return null;
  const bbox = await maybeBoundingBox(page, selector);
  if (!bbox) return null;
  const x = bbox.x + bbox.width * 0.5;
  const y = bbox.y + bbox.height * 0.5;
  await page.mouse.move(x, y, { steps: 10 });
  return { bbox, x, y };
}

function recordFromAction(action, actionType, tMs, durationMs, status, error, bbox, cursor, screenshotPath) {
  return {
    id: asString(action.id),
    action_type: actionType,
    status,
    error: error || null,
    t_ms: Math.max(0, Math.round(tMs)),
    duration_ms: Math.max(0, Math.round(durationMs)),
    selector: typeof action.selector === 'string' ? action.selector : null,
    cursor_x: Number.isFinite(cursor?.x) ? cursor.x : null,
    cursor_y: Number.isFinite(cursor?.y) ? cursor.y : null,
    target_x: Number.isFinite(bbox?.x) ? bbox.x : null,
    target_y: Number.isFinite(bbox?.y) ? bbox.y : null,
    target_w: Number.isFinite(bbox?.width) ? bbox.width : null,
    target_h: Number.isFinite(bbox?.height) ? bbox.height : null,
    screenshot_path: screenshotPath || null
  };
}

async function runActions(payload, cwd) {
  const web = payload?.web || {};
  const baseUrl = asString(web.base_url).trim();
  const viewport = web.viewport && Number.isFinite(web.viewport.width) && Number.isFinite(web.viewport.height)
    ? { width: Number(web.viewport.width), height: Number(web.viewport.height) }
    : { width: 1440, height: 900 };
  const actions = Array.isArray(web.actions) ? web.actions : [];

  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport, deviceScaleFactor: 1 });
  const page = await context.newPage();
  const startedAt = Date.now();
  const records = [];
  let cursor = { x: viewport.width * 0.5, y: viewport.height * 0.5 };
  let firstError = null;

  try {
    for (let index = 0; index < actions.length; index += 1) {
      const action = actions[index];
      const actionType = asString(action.type);
      const actionStart = Date.now();
      let status = 'ok';
      let error = null;
      let screenshotPath = null;

      try {
        switch (actionType) {
          case 'goto': {
            const targetUrl = toUrl(action.url, baseUrl);
            if (!targetUrl) throw new Error('goto action requires url');
            await page.goto(targetUrl, { waitUntil: 'domcontentloaded', timeout: 15000 });
            break;
          }
          case 'click': {
            const selector = asString(action.selector).trim();
            if (!selector) throw new Error('click action requires selector');
            await moveMouseToTarget(page, selector);
            await page.locator(selector).first().click({ timeout: 10000 });
            break;
          }
          case 'type': {
            const selector = asString(action.selector).trim();
            const text = asString(action.text);
            if (!selector) throw new Error('type action requires selector');
            await moveMouseToTarget(page, selector);
            await page.locator(selector).first().click({ timeout: 10000 });
            await page.keyboard.type(text, { delay: 38 });
            break;
          }
          case 'press': {
            const key = asString(action.key).trim();
            if (!key) throw new Error('press action requires key');
            await page.keyboard.press(key);
            break;
          }
          case 'wait_for_selector': {
            const selector = asString(action.selector).trim();
            if (!selector) throw new Error('wait_for_selector action requires selector');
            await page.waitForSelector(selector, { state: 'visible', timeout: 15000 });
            break;
          }
          case 'wait_ms': {
            const ms = Number(action.wait_ms || 0);
            if (!(ms > 0)) throw new Error('wait_ms action requires wait_ms > 0');
            await page.waitForTimeout(ms);
            break;
          }
          case 'assert_text': {
            const text = asString(action.text);
            if (!text) throw new Error('assert_text action requires text');
            const bodyText = await page.locator('body').innerText();
            if (!bodyText.includes(text)) {
              throw new Error(`assert_text failed: '${text}' not found`);
            }
            break;
          }
          case 'screenshot': {
            const outputPathRaw = asString(action.path).trim();
            if (!outputPathRaw) throw new Error('screenshot action requires path');
            screenshotPath = path.resolve(cwd, outputPathRaw);
            await fs.mkdir(path.dirname(screenshotPath), { recursive: true });
            // Keep screenshots viewport-scoped so web video framing remains consistent.
            await page.screenshot({ path: screenshotPath, fullPage: false });
            break;
          }
          case 'scroll_to': {
            const selector = asString(action.selector).trim();
            if (!selector) throw new Error('scroll_to action requires selector');
            await page.locator(selector).first().scrollIntoViewIfNeeded();
            break;
          }
          default:
            throw new Error(`unsupported web action type: ${actionType}`);
        }
      } catch (err) {
        status = 'failed';
        error = err?.message || String(err);
        if (!firstError) firstError = error;
      }

      // Always capture a frame after each action so the renderer has concrete web visuals.
      if (!screenshotPath) {
        try {
          screenshotPath = await captureActionScreenshot(
            page,
            cwd,
            asString(action.id),
            index,
          );
        } catch {
          screenshotPath = null;
        }
      }

      const bbox = await maybeBoundingBox(page, asString(action.selector).trim());
      if (bbox) {
        cursor = { x: bbox.x + bbox.width * 0.5, y: bbox.y + bbox.height * 0.5 };
      }

      const tMs = actionStart - startedAt;
      const durationMs = Date.now() - actionStart;
      records.push(recordFromAction(action, actionType, tMs, durationMs, status, error, bbox, cursor, screenshotPath));

      if (status !== 'ok') break;
    }
  } finally {
    await browser.close();
  }

  return {
    ok: !records.some((r) => r.status !== 'ok'),
    actions: records,
    error: firstError
  };
}

async function main() {
  const args = parseArgs(process.argv);
  if (!args.config || !args.output) {
    throw new Error('usage: node web-runner.mjs --config <path> --output <path> [--cwd <path>]');
  }

  const configPath = path.resolve(args.config);
  const outputPath = path.resolve(args.output);
  const cwd = args.cwd ? path.resolve(args.cwd) : process.cwd();

  const payload = JSON.parse(await fs.readFile(configPath, 'utf8'));
  const result = await runActions(payload, cwd);
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, JSON.stringify(result), 'utf8');
  process.stdout.write(JSON.stringify({ ok: result.ok, actions: result.actions.length }) + '\n');
  process.exit(result.ok ? 0 : 1);
}

main().catch((err) => {
  console.error(err?.stack || err?.message || String(err));
  process.exit(1);
});
