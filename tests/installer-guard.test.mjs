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
const section = (text, name) => {
  const marker = `Section ${name}\n`;
  const start = text.indexOf(marker);
  assert.notEqual(start, -1, `section ${name}`);
  return text.slice(start + marker.length, text.indexOf('SectionEnd', start));
};
const before = (text, first, second) => {
  const a = text.indexOf(first);
  const b = text.indexOf(second);
  assert.notEqual(a, -1, `missing ${first}`);
  assert.notEqual(b, -1, `missing ${second}`);
  assert.ok(a < b, `${first} must precede ${second}`);
};

test('installer prepares V2 on its worker after choices, while uninstall retains V1 denial', async () => {
  const text = await read('installer.nsi');
  assert.doesNotMatch(body(text, '.onInit'), /Call FBForgeAcquire|Call FBForgePrepareV2/);
  assert.doesNotMatch(text, /MUI_PAGE_DIRECTORY/);
  before(text, 'Page custom FBForgeDesktopPage', '!insertmacro MUI_PAGE_INSTFILES');
  before(text, 'Section "-FBForge native preparation"', 'Section WebView2');
  const preparing = section(text, '"-FBForge native preparation"');
  before(preparing, 'Call FBForgeExtractPayload', 'Call FBForgePrepareV2');
  assert.ok(
    body(text, 'un.onInit').startsWith(
      '  Call un.FBForgeAcquire\n  Call un.FBForgeRequireMutationEvidence\n',
    ),
  );
  assert.ok(section(text, 'Uninstall').startsWith('  Call un.FBForgeRequireMutationEvidence'));
  const gate = body(await read('guard.nsh'), '${PREFIX}FBForgeRequireMutationEvidence');
  assert.ok(gate.includes('Call ${PREFIX}FBForgeRequireHeld'));
  assert.match(gate, /SetErrorLevel 70[\s\S]*Quit/);
  assert.doesNotMatch(gate, /Return|!if|!else|TODO|success/i);
});

test('V2 phases protect extraction and recovery metadata without forcing application exit', async () => {
  const all = (
    await Promise.all(['installer.nsi', 'guard.nsh', 'guard-v2.nsh', 'safe-utils.nsh'].map(read))
  ).join('\n');
  assert.doesNotMatch(
    all,
    /CheckIfAppIsRunning|RmForceShutdown|RmShutdown|TerminateProcess|ExecWait ['"`]\$R1|_\?=/,
  );
  const text = await read('installer.nsi');
  const installing = section(text, 'Install');
  before(installing, 'Call FBForgeRequireStagingV2', 'File "${MAINBINARYSRCPATH}"');
  for (const phase of ['publishedPending', 'recoveryPublishedPending']) {
    before(
      installing,
      `StrCmpS $FBForgeV2Phase "v2:${phase}" published_metadata`,
      'Call FBForgeRequireStagingV2',
    );
  }
  before(installing, 'Call FBForgePublishV2', 'published_metadata:');
  before(installing, 'published_metadata:', 'Call FBForgeRequirePublishedV2');
  before(installing, 'Call FBForgeRequirePublishedV2', 'WriteRegStr');
  const prerequisite = section(text, 'WebView2');
  before(
    prerequisite,
    'Call FBForgeRequireStagingV2',
    'File "/oname=.fbforge-webview2-prerequisite.exe"',
  );
  before(prerequisite, 'ExecWait', 'Delete "$FBForgeV2Stage\\.fbforge-webview2-prerequisite.exe"');
  assert.doesNotMatch(prerequisite, /NSISdl::|inetc::|ExecWait[^\n]*EdgeUpdate/);
});

test('worker finalizes metadata then fresh release ACK before either optional launch path', async () => {
  const text = await read('installer.nsi');
  const installing = section(text, 'Install');
  before(installing, 'Call CreateOrUpdateDesktopShortcut', 'Call FBForgeFinalizeV2');
  before(installing, '"Installer Language" $LANGUAGE', 'Call FBForgeFinalizeV2');
  for (const name of ['RunMainBinary', '.onInstSuccess']) {
    const launching = body(text, name);
    before(launching, 'Call FBForgeRequireCompletedV2', 'RunAsUser');
    assert.doesNotMatch(launching, /Call FBForgeFinalizeV2/);
  }
  assert.match(
    body(text, 'CreateOrUpdateDesktopShortcut'),
    /\$FBForgeShortcutsFinalized = 1[\s\S]*Return/,
  );
  const v2 = await read('guard-v2.nsh');
  const finalizing = body(v2, 'FBForgeFinalizeV2');
  before(finalizing, 'Call FBForgeRequirePublishedV2', '::FinalizeV2 /NOUNLOAD');
  before(finalizing, 'Call FBForgeWaitV2', '::ReleaseV2 /NOUNLOAD');
  before(
    finalizing,
    'StrCmpS $FBForgeGuardReply "v2:released" 0 denied',
    'StrCpy $FBForgeGuardReleased 1',
  );
  before(finalizing, 'StrCpy $FBForgeGuardReleased 1', 'SetErrorLevel 0');
  const complete = body(v2, 'FBForgeRequireCompletedV2');
  assert.ok(complete.includes('StrCmpS $FBForgeV2Complete 1 0 denied'));
  assert.ok(complete.includes('StrCmpS $FBForgeGuardReleased 1 0 denied'));
  before(complete, 'Call FBForgePollV2', 'StrCmpS $FBForgeV2Phase "v2:released" 0 denied');
});

test('V2 rejects stale or malformed replies and failure never releases or starts an app', async () => {
  const v2 = await read('guard-v2.nsh');
  const reply = body(v2, 'FBForgeV2ReadReply');
  assert.ok(reply.startsWith('  IfErrors denied\n'));
  before(reply, 'Pop $FBForgeGuardReply', 'Pop $FBForgeGuardSentinel');
  assert.ok(reply.includes('StrCmpS $FBForgeGuardSentinel "fbforge-guard-v2-sentinel" 0 denied'));
  const wait = body(v2, 'FBForgeWaitV2');
  assert.ok(wait.includes('IntCmp $FBForgeV2PollCount ${FBFORGE_V2_MAX_POLLS} 0 0 denied'));
  assert.ok(wait.includes('StrCmpS $FBForgeV2Phase "v2:complete" ready denied'));
  const failure = body(v2, 'FBForgeInstallFailed');
  assert.match(failure, /StrCpy \$FBForgeInstallFailed 1[\s\S]*SetErrorLevel 70[\s\S]*Quit/);
  assert.doesNotMatch(failure, /ReleaseV2|RunAsUser/);
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
