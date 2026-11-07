#!/usr/bin/env node
// Takes the README screenshots (docs/screenshots/*.png) of the built desktop app through tauri-driver (WebDriver),
// the same way e2e/smoke.mjs drives it: a profile of its own, file dialogs answered from the page
// (window.__PPR_DIALOG_ANSWERS__, see src/lib/tauriBackend.ts), and on Linux a virtual display, so no window appears.
//
//   pnpm tauri build --debug --no-bundle
//   PPR_SHOT_ROM=~/roms/emerald.gba pnpm screenshots
//
// Environment:
//   PPR_SHOT_ROM   Pokémon Emerald (required); the shots show its options
//   PPR_SHOT_APP   the app to drive (default: target/debug/potato-pokemon-randomizer)
//   PPR_SHOT_OUT   where the PNGs go (default: docs/screenshots)
//   PPR_SHOT_TMP   parent of the throwaway profile and save folder (default: ~/.cache/ppr-tmp); both are deleted
//   PPR_SHOT_SEED  the seed of the saved game (default: 123456789)
//
// Needs Linux with Xvfb, `tauri-driver` (cargo install tauri-driver) and WebKitWebDriver (Fedora: webkitgtk6.0;
// Debian/Ubuntu: webkit2gtk-driver). The window is 1280×800 in the dark theme. Clicks go through the page rather
// than the pointer, and transitions, animations and the text caret are switched off, so no hover or half-finished
// state ends up in a shot. The randomization uses a fixed seed (through the Premade Seed dialog), so every run shows
// the same result. The saved dialog shows the save path with the home folder written as ~.
//
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const root = resolve(import.meta.dirname, '..');
const app = process.env.PPR_SHOT_APP ?? join(root, 'target', 'debug', 'potato-pokemon-randomizer');
const rom = process.env.PPR_SHOT_ROM;
const outDir = resolve(process.env.PPR_SHOT_OUT ?? join(root, 'docs', 'screenshots'));
const tmpBase = process.env.PPR_SHOT_TMP ?? join(homedir(), '.cache', 'ppr-tmp');
const seed = process.env.PPR_SHOT_SEED ?? '123456789';
const DRIVER = 'http://127.0.0.1:4444';
const ELEMENT = 'element-6066-11e4-a52e-4f735466cecf';
const WIDTH = 1280;
const HEIGHT = 800;
const STARTERS = ['Pikachu', 'Eevee', 'Bagon'];

if (process.platform !== 'linux' || !hasCommand('Xvfb')) {
  console.error('The screenshots need Linux with Xvfb, so they look the same on every run.');
  process.exit(1);
}
if (!rom || !existsSync(rom)) {
  console.error('Set PPR_SHOT_ROM to a Pokémon Emerald ROM.');
  process.exit(1);
}
if (!existsSync(app)) {
  console.error(`No app at ${app}. Build it with: pnpm tauri build --debug --no-bundle`);
  process.exit(1);
}

mkdirSync(tmpBase, { recursive: true });
mkdirSync(outDir, { recursive: true });
// A fixed folder, so the saved dialog shows the same path on every run.
const work = join(tmpBase, 'ppr-screenshots');
rmSync(work, { recursive: true, force: true });
const home = mkdtempSync(join(tmpBase, 'ppr-screenshots-profile-'));
mkdirSync(work, { recursive: true });
const output = join(work, 'Emerald-randomized' + extname(rom));

const children = [];
const env = { ...process.env, XDG_CONFIG_HOME: join(home, 'config'), XDG_DATA_HOME: join(home, 'data'), XDG_CACHE_HOME: join(home, 'cache') };
// A screen larger than the window, with the window in its top-left corner: the pointer starts in the middle of the
// screen, outside the window, so nothing shows a hover state.
const display = ':' + (90 + Math.floor(Math.random() * 9));
children.push(spawn('Xvfb', [display, '-screen', '0', '2800x1800x24', '-nolisten', 'tcp'], { stdio: 'ignore' }));
Object.assign(env, { DISPLAY: display, GDK_BACKEND: 'x11', GDK_SCALE: '1' });
delete env.WAYLAND_DISPLAY;
await sleep(500);
children.push(spawn('tauri-driver', [], { env, stdio: ['ignore', 'ignore', 'inherit'] }));

let session;
let failed = false;

try {
  await waitFor(async () => (await fetch(DRIVER + '/status').catch(() => null))?.ok, 'tauri-driver', 15_000);
  session = (await wd('POST', '/session', { capabilities: { alwaysMatch: { 'tauri:options': { application: app } } } })).sessionId;
  await waitFor(() => script('return !!window.__TAURI_INTERNALS__ && document.readyState === "complete"'), 'the app', 30_000);
  await wd('POST', `/session/${session}/window/rect`, { x: 0, y: 0, width: WIDTH, height: HEIGHT });
  await waitFor(async () => (await script('return [innerWidth, innerHeight]')).join() === `${WIDTH},${HEIGHT}`, `a ${WIDTH}×${HEIGHT} window`, 5_000);
  await script(`const s = document.createElement('style');
    s.textContent = '*, *::before, *::after { transition: none !important; animation: none !important; caret-color: transparent !important; }';
    document.head.append(s);`);
  // The ROM to open, then the ROM for the premade seed, then where to save the randomized game.
  await script('window.__PPR_DIALOG_ANSWERS__ = arguments[0]', [[rom, rom, output]]);

  // 1. The start screen.
  await find("//button[contains(., 'Open ROM')]", 15_000);
  await shot('start.png');

  // 2. The randomizer: the ROM open on the Starters tab, custom starters and a few options.
  await click(await find("//button[contains(., 'Open ROM')]"));
  await find("//section[.//h2[normalize-space()='Starter Pokémon']]", 30_000);
  step('opened ' + rom);
  await pickMode('Starter Pokémon', 'Custom');
  for (const [i, name] of STARTERS.entries()) {
    await click(await find(`//button[starts-with(@aria-label, 'Starter ${i + 1}:')]`));
    await type(await find("//*[@role='dialog']//input[@type='search']"), name);
    await click(await find(`//button[contains(@class, 'dexbtn')][.//*[contains(@class, 'dl')][normalize-space()='${name}']]`));
    await waitGone("//*[@role='dialog']");
  }
  await toggle('Randomize Starter Held Items');
  await toggle('Ban Bad Items');
  await shot('randomizer.png');

  // 3. The starter picker.
  await click(await find("//button[starts-with(@aria-label, 'Starter 2:')]"));
  await find("//*[@role='dialog']//button[contains(@class, 'dexbtn')]");
  await shot('starter-picker.png');
  await closeDialog();

  // 4. The batch randomization dialog, from the Tools menu.
  await click(await find("//header//button[@aria-label='Tools']"));
  await click(await find("//button[@role='menuitem'][normalize-space()='Batch Randomization Settings']"));
  await find("//*[@role='dialog']");
  await shot('batch.png');
  await closeDialog();

  // The settings string of these options, for the premade seed below; this dialog isn't one of the shots.
  await click(await find("//footer//button[normalize-space()='Settings String']"));
  const current = await find("//input[@aria-label='Current settings']");
  let settingsString = '';
  await waitFor(async () => (settingsString = await script('return arguments[0].value', [ref(current)])).length > 0, 'the settings string', 10_000);
  await closeDialog();

  // 5. Randomize and save with a fixed seed, through the Premade Seed dialog, and show the saved dialog.
  await click(await find("//footer//button[normalize-space()='Premade Seed']"));
  await type(await find("//label[.//span[normalize-space()='Random seed']]//input"), seed);
  await type(await find("//label[.//span[normalize-space()='Settings string']]//input"), settingsString);
  const browse = "//div[contains(@class, 'field')][.//span[normalize-space()='ROM file']]//button";
  await waitFor(async () => (await script('return !arguments[0].disabled', [ref(await find(browse))])), 'the premade seed to be read', 10_000);
  await click(await find(browse));
  const apply = "//button[normalize-space()='Apply Randomization Settings']";
  await waitFor(async () => (await script('return !arguments[0].disabled', [ref(await find(apply))])), 'the ROM to match', 30_000);
  await click(await find(apply));
  await find("//*[contains(text(), 'Your game has been randomized')]", 180_000);
  step('randomized with seed ' + seed);
  // The save path, without the home folder of whoever runs this.
  await script(`for (const el of document.querySelectorAll('[role=dialog] .lede')) el.textContent = el.textContent.split(arguments[0]).join('~');`, [homedir()]);
  await shot('saved.png');
  await click(await find("//button[normalize-space(.)='Done']"));
  await waitGone("//*[@role='dialog']");

  // 6. Manual edits, Trainers section, with two edits made. A finished run goes back to the start screen, so the ROM
  // opens again from the recent list.
  if (!(await exists("//header//button[contains(normalize-space(.), 'Manual edits')]"))) {
    await click(await find("//button[contains(., 'Pokémon Emerald')]"));
    await find("//section[.//h2[normalize-space()='Starter Pokémon']]", 30_000);
  }
  await click(await find("//header//button[contains(normalize-space(.), 'Manual edits')]"));
  await click(await find("//nav//button[contains(normalize-space(.), 'Trainers')]"));
  await manualEdits();
  // Below 1440 px the Changes list is a drawer; close it so the section shows.
  await script("window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))");
  await waitGone("//*[contains(@class, 'scrim')][not(contains(@style, 'display: none'))]", 5_000).catch(() => {});
  await shot('manual-edits.png');
  console.log(`\nScreenshots written to ${outDir}`);
} catch (e) {
  failed = true;
  console.error('\nScreenshots failed:', e instanceof Error ? e.message : e);
  if (session) {
    const page = await script('return document.body.innerText').catch(() => '');
    console.error('\nThe window showed:\n' + String(page).slice(0, 1500));
  }
} finally {
  if (session) await wd('DELETE', `/session/${session}`).catch(() => {});
  // Let the app and the driver exit first, or the app may write its profile again after it is deleted.
  await Promise.all(children.reverse().map((c) => new Promise((done) => (c.exitCode !== null ? done() : (c.once('exit', done), c.kill())))));
  await sleep(1000);
  rmSync(work, { recursive: true, force: true });
  rmSync(home, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);

/** Makes two edits in the open Manual edits section: the first two pickers each get the fifth choice offered. */
async function manualEdits() {
  for (let i = 1; i <= 2; i++) {
    const picker = `(//main//button[contains(@class, 'pickbtn')])[${i}]`;
    if (!(await exists(picker))) {
      console.log(`- manual-edits.png: no picker #${i} to edit in the Trainers section`);
      return;
    }
    await click(await find(picker));
    await click(await find("(//*[@role='dialog']//button[contains(@class, 'dexbtn')])[5]"));
    await waitGone("//*[@role='dialog']");
  }
}

function step(what) {
  console.log('✓ ' + what);
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
function ref(id) {
  return { [ELEMENT]: id };
}
async function find(xpath, ms = 10_000) {
  let found;
  await waitFor(async () => {
    found = (await wd('POST', `/session/${session}/element`, { using: 'xpath', value: xpath }))[ELEMENT];
    return !!found;
  }, xpath, ms);
  return found;
}
async function exists(xpath) {
  return (await wd('POST', `/session/${session}/elements`, { using: 'xpath', value: xpath })).length > 0;
}
function waitGone(xpath, ms = 10_000) {
  return waitFor(async () => !(await exists(xpath)), 'no ' + xpath, ms);
}
/** Clicks through the page, so the pointer stays put and nothing gets a hover state. */
function click(id) {
  return script('arguments[0].click()', [ref(id)]);
}
async function type(id, text) {
  await wd('POST', `/session/${session}/element/${id}/clear`, {});
  await wd('POST', `/session/${session}/element/${id}/value`, { text });
}
async function pickMode(group, choice) {
  await click(await find(`//section[.//h2[normalize-space()='${group}']]//button[@role='radio'][normalize-space()='${choice}']`));
}
async function toggle(label) {
  await click(await find(`//label[contains(@class, 'row')][.//span[normalize-space()='${label}']]//input[@type='checkbox']`));
}
async function closeDialog() {
  await script("window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))");
  await waitGone("//*[@role='dialog']");
}
/** Waits until the window is calm (no toast, no focus ring, fonts loaded), then saves a screenshot. */
async function shot(name) {
  await waitGone("//*[contains(@class, 'toast')]", 8_000);
  await script('document.activeElement?.blur()');
  await script('return document.fonts.ready.then(() => true)').catch(() => true);
  await sleep(400);
  const png = Buffer.from(await wd('GET', `/session/${session}/screenshot`), 'base64');
  writeFileSync(join(outDir, name), png);
  step(name);
  const scrolls = await script(`return [...document.querySelectorAll('.sheet')]
    .some((e) => e.scrollHeight > e.clientHeight + 1 && ['auto', 'scroll'].includes(getComputedStyle(e).overflowY))`);
  if (scrolls) console.log(`! ${name}: the dialog doesn't fit the window, so part of it is cut off`);
}
