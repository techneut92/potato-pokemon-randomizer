#!/usr/bin/env node
// End-to-end smoke test of the desktop app through tauri-driver (WebDriver): open a ROM, pick random starters,
// randomize, save the ROM and its log. File dialogs take answers the test puts in the page (tauriBackend.ts).
//
//   pnpm tauri build --debug --no-bundle
//   PPR_E2E_ROM=~/roms/emerald.gba pnpm e2e
//
// Needs `tauri-driver` (cargo install tauri-driver) and the platform's WebDriver: WebKitWebDriver on Linux
// (Fedora: webkitgtk6.0; Debian/Ubuntu: webkit2gtk-driver), msedgedriver on Windows. macOS has no WebDriver for
// WKWebView. On Linux the app runs on a virtual display when Xvfb is installed, so no window appears.
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const root = resolve(import.meta.dirname, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const app = process.env.PPR_E2E_APP ?? join(root, 'target', 'debug', 'potato-pokemon-randomizer' + exe);
const rom = process.env.PPR_E2E_ROM;
const DRIVER = 'http://127.0.0.1:4444';

if (!rom || !existsSync(rom)) {
  console.log('PPR_E2E_ROM is not set to a Gen 3 ROM; skipping the end-to-end test.');
  process.exit(0);
}
if (!existsSync(app)) {
  console.error(`No app at ${app}. Build it with: pnpm tauri build --debug --no-bundle`);
  process.exit(1);
}

const children = [];
// A profile of its own, so the test leaves the user's recent ROMs, window state and custom names alone.
const home = mkdtempSync(join(tmpdir(), 'ppr-e2e-home-'));
const env = { ...process.env, XDG_CONFIG_HOME: join(home, 'config'), XDG_DATA_HOME: join(home, 'data'), XDG_CACHE_HOME: join(home, 'cache') };
if (process.platform === 'linux' && hasCommand('Xvfb')) {
  const display = ':' + (90 + Math.floor(Math.random() * 9));
  children.push(spawn('Xvfb', [display, '-screen', '0', '1280x900x24'], { stdio: 'ignore' }));
  Object.assign(env, { DISPLAY: display, GDK_BACKEND: 'x11' });
  delete env.WAYLAND_DISPLAY;
  await sleep(500);
}
const driver = spawn('tauri-driver', [], { env, stdio: ['ignore', 'ignore', 'inherit'] });
children.push(driver);

const out = mkdtempSync(join(tmpdir(), 'ppr-e2e-'));
const output = join(out, 'randomized.gba');
let session;
let failed = false;

try {
  await waitFor(async () => (await fetch(DRIVER + '/status').catch(() => null))?.ok, 'tauri-driver', 15_000);
  session = (await wd('POST', '/session', { capabilities: { alwaysMatch: { 'tauri:options': { application: app } } } })).sessionId;

  // Answer the file dialogs in order (see tauriBackend.ts): the ROM to open, then where to save it.
  await waitFor(() => script('return !!window.__TAURI_INTERNALS__'), 'the app', 30_000);
  await script('window.__PPR_DIALOG_ANSWERS__ = arguments[0]', [[rom, output]]);

  await click(await find("//button[contains(., 'Open ROM')]"));
  const starters = await find("//*[normalize-space(text())='Random (completely)']", 30_000);
  step('opened ' + rom);
  await click(starters);
  await click(await find("//footer//button[contains(., 'Randomize & Save')]"));
  await find("//*[contains(text(), 'Save a log of this randomization')]", 120_000);
  step('randomized');
  await click(await find("//*[contains(text(), 'Save a log of this randomization')]"));
  await click(await find("//button[normalize-space(.)='Done']"));
  await waitFor(() => readdirSync(out).some((f) => f.endsWith('.log')), 'the log', 15_000);

  const written = readFileSync(output);
  const original = readFileSync(rom);
  check(written.length === original.length, 'the randomized ROM has the original size');
  check(!written.equals(original), 'the randomized ROM differs from the original');
  const log = readFileSync(join(out, readdirSync(out).find((f) => f.endsWith('.log'))), 'utf8');
  check(log.includes('Starter Pokemon'), 'the log lists the starters');
  step(`saved ${output} (${statSync(output).size} bytes) and its log`);
  console.log('\nEnd-to-end test passed.');
} catch (e) {
  failed = true;
  console.error('\nEnd-to-end test failed:', e instanceof Error ? e.message : e);
  if (session) {
    const page = await script('return document.body.innerText').catch(() => '');
    console.error('\nThe window showed:\n' + String(page).slice(0, 1500));
  }
} finally {
  if (session) await wd('DELETE', `/session/${session}`).catch(() => {});
  for (const c of children.reverse()) c.kill();
  rmSync(out, { recursive: true, force: true });
  rmSync(home, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);

function step(what) {
  console.log('✓ ' + what);
}
function check(ok, what) {
  if (!ok) throw new Error('expected: ' + what);
  step(what);
}
function hasCommand(cmd) {
  return (process.env.PATH ?? '').split(':').some((dir) => existsSync(join(dir, cmd)));
}
async function wd(method, path, body) {
  const res = await fetch(DRIVER + path, { method, headers: { 'content-type': 'application/json' }, body: body && JSON.stringify(body) });
  const json = await res.json();
  if (!res.ok) throw new Error(`${method} ${path}: ${json.value?.message ?? res.status}`);
  return json.value;
}
async function waitFor(cond, what, ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await Promise.resolve(cond()).catch(() => false)) return;
    await sleep(200);
  }
  throw new Error('timed out waiting for ' + what);
}
function script(body, args = []) {
  return wd('POST', `/session/${session}/execute/sync`, { script: body, args });
}
async function find(xpath, ms = 10_000) {
  let found;
  await waitFor(async () => {
    const el = await wd('POST', `/session/${session}/element`, { using: 'xpath', value: xpath });
    found = Object.values(el)[0];
    return true;
  }, xpath, ms);
  return found;
}
function click(id) {
  return wd('POST', `/session/${session}/element/${id}/click`, {});
}
