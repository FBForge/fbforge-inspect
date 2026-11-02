import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('signing gate rejects failed, unrelated and unapproved source before using the signing runner', () => {
  const source = fileURLToPath(new URL('../scripts/ci/check-signing-run.py', import.meta.url));
  const program = `
import copy, importlib.util, os, sys
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('signing_gate', sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
commit = 'a' * 40
run = {'head_sha': commit, 'head_branch': 'main', 'head_repository': {'full_name': module.REPOSITORY},
       'path': '.github/workflows/verify.yml', 'event': 'push', 'status': 'completed', 'conclusion': 'success'}
jobs = [{'name': 'windows', 'conclusion': 'success'}, {'name': 'verify', 'conclusion': 'success'}]
environment = {'protection_rules': [{'type': 'required_reviewers', 'prevent_self_review': False,
                'reviewers': [{'type': 'User', 'reviewer': {'login': 'FBForge'}}]}]}
module.validate(run, jobs, environment, commit, 'main')
def rejected(fn, expected):
    try: fn()
    except RuntimeError as error: assert str(error) == expected, str(error)
    else: raise AssertionError('unsafe signing request accepted: ' + expected)
for key, value, reason in [
    ('head_sha', 'b' * 40, 'signing_build_commit_mismatch'),
    ('head_branch', 'release/0.2.0', 'signing_build_branch_mismatch'),
    ('head_repository', {'full_name': 'outsider/fork'}, 'signing_build_repository_mismatch'),
    ('path', '.github/workflows/other.yml', 'signing_build_workflow_mismatch'),
    ('event', 'pull_request', 'signing_build_event_denied'),
    ('status', 'in_progress', 'signing_build_not_successful'),
    ('conclusion', 'failure', 'signing_build_not_successful'),
    ('conclusion', 'cancelled', 'signing_build_not_successful'),
]:
    changed = dict(run, **{key: value})
    rejected(lambda: module.validate(changed, jobs, environment, commit, 'main'), reason)
rejected(lambda: module.validate(run, [], environment, commit, 'main'), 'signing_verify_validation_missing')
rejected(lambda: module.validate(run, jobs * 2, environment, commit, 'main'), 'signing_verify_validation_missing')
rejected(lambda: module.validate(run, jobs, {}, commit, 'main'), 'signing_environment_approval_missing')
rejected(lambda: module.validate(run, jobs, environment, 'not-a-commit', 'main'), 'signing_commit_invalid')
rejected(lambda: module.validate(run, jobs, environment, commit, 'feature/untrusted'), 'signing_branch_denied')
rejected(lambda: module.validate(run, jobs, environment, commit, 'develop'), 'signing_branch_denied')
changed = copy.deepcopy(environment)
changed['protection_rules'][0]['reviewers'][0]['reviewer']['login'] = 'outsider'
rejected(lambda: module.validate(run, jobs, changed, commit, 'main'), 'signing_environment_owner_required')
changed = copy.deepcopy(environment)
changed['protection_rules'][0]['prevent_self_review'] = True
rejected(lambda: module.validate(run, jobs, changed, commit, 'main'), 'signing_owner_cannot_approve_own_run')
os.environ.update(GITHUB_REPOSITORY=module.REPOSITORY, GITHUB_EVENT_NAME='workflow_dispatch',
                  GITHUB_REF='refs/heads/main', GITHUB_SHA=commit, VALIDATION_RUN='123')
calls = []
def api(path):
    calls.append(path)
    if '/environments/' in path: return environment
    if '/jobs?' in path: return {'jobs': jobs, 'total_count': 2}
    return run
module.api = api
module.main()
assert len(calls) == 3
for key, value, reason in [
    ('GITHUB_EVENT_NAME', 'pull_request', 'signing_event_denied'),
    ('GITHUB_REPOSITORY', 'outsider/fork', 'signing_repository_denied'),
    ('VALIDATION_RUN', '123;echo injected', 'signing_run_id_invalid'),
]:
    previous = os.environ[key]
    os.environ[key] = value
    calls.clear()
    rejected(module.main, reason)
    assert calls == []
    os.environ[key] = previous
print('signing trust boundary passed')
`;
  const result = execFileSync(
    process.platform === 'win32' ? 'python' : 'python3',
    ['-c', program, source],
    {
      encoding: 'utf8',
      timeout: 10000,
    },
  );
  assert.match(result, /signing trust boundary passed/);
});
