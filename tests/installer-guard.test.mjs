import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
const read = (name) => readFile(new URL(`../desktop/installer/${name}`, import.meta.url), 'utf8');
const body = (text, name) => {
  const start = text.indexOf(`Function ${name}\n`);
  assert.notEqual(start, -1, `function ${name}`);
  const from = start + `Function ${name}\n`.length;
  return text.slice(from, text.indexOf('FunctionEnd', from));
};
test('first installer and uninstaller action acquires before the missing native evidence gate', async () => {
  const text = await read('installer.nsi');
  for (const prefix of ['', 'un.']) {
    const init = body(text, prefix ? 'un.onInit' : '.onInit');
    assert.ok(
      init.startsWith(
        `  Call ${prefix}FBForgeAcquire\n  Call ${prefix}FBForgeRequireMutationEvidence\n`,
      ),
    );
  }
  const guard = await read('guard.nsh');
  assert.ok(guard.includes('!define FBFORGE_GUARD_APP_ID "de.fbforge.inspect"'));
  const gate = body(guard, '${PREFIX}FBForgeRequireMutationEvidence');
  assert.ok(gate.includes('Call ${PREFIX}FBForgeRequireHeld'));
  assert.match(gate, /SetErrorLevel 70[\s\S]*Quit/);
  assert.doesNotMatch(gate, /Return|!if|!else|TODO|success/i);
});
test('protected phases contain no forced shutdown or nested uninstaller delegation', async () => {
  const all = (await Promise.all(['installer.nsi', 'guard.nsh', 'safe-utils.nsh'].map(read))).join(
    '\n',
  );
  assert.doesNotMatch(
    all,
    /CheckIfAppIsRunning|RmForceShutdown|RmShutdown|TerminateProcess|ExecWait ['"`]\$R1|_\?=/,
  );
  const text = await read('installer.nsi');
  for (const section of ['WebView2', 'Install', 'Uninstall']) {
    const prefix = section === 'Uninstall' ? 'un.' : '';
    assert.ok(text.includes(`Section ${section}\n  Call ${prefix}FBForgeRequireMutationEvidence`));
  }
});
test('finalizer owns shortcut mutation then exact release, all launch paths finalize first', async () => {
  const text = await read('installer.nsi');
  assert.match(text, /MUI_FINISHPAGE_SHOWREADME_FUNCTION FBForgeFinalizeInstall/);
  assert.match(text, /MUI_PAGE_CUSTOMFUNCTION_LEAVE FBForgeFinalizeInstall/);
  const finalizer = body(text, 'FBForgeFinalizeInstall');
  assert.ok(
    finalizer.indexOf('Call CreateOrUpdateDesktopShortcut') <
      finalizer.indexOf('Call FBForgeRelease'),
  );
  assert.match(body(text, 'RunMainBinary'), /Call FBForgeFinalizeInstall[\s\S]*RunAsUser/);
  assert.match(body(text, '.onInstSuccess'), /Call FBForgeFinalizeInstall[\s\S]*RunAsUser/);
  assert.match(
    body(text, 'CreateOrUpdateDesktopShortcut'),
    /\$FBForgeShortcutsFinalized = 1[\s\S]*Return/,
  );
});

test('the configured template and fixed native identity stay paired to pinned CLI', async () => {
  const config = JSON.parse(
    await readFile(new URL('../desktop/tauri.conf.json', import.meta.url), 'utf8'),
  );
  const pkg = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  const upstream = JSON.parse(await read('upstream.json'));
  const guard = await read('guard.nsh');
  assert.equal(pkg.devDependencies['@tauri-apps/cli'], upstream.version);
  assert.equal(config.bundle.windows.nsis.template, 'installer/installer.nsi');
  assert.ok(guard.includes(`!define FBFORGE_GUARD_APP_ID "${config.identifier}"`));
});
