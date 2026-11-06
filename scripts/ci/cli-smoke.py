"""Exercise the portable CLI against local public fixtures on Windows and Linux."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('engine', type=Path)
    parser.add_argument('--java', default='java')
    args = parser.parse_args()
    engine = args.engine.resolve()
    java = str(Path(args.java).resolve()) if Path(args.java).is_file() else args.java
    environment = {key: value for key, value in os.environ.items()
                   if key.upper() in {'PATH', 'SYSTEMROOT', 'WINDIR', 'TEMP', 'TMP'}}
    with tempfile.TemporaryDirectory(prefix='inspect-cli-smoke-') as directory:
        root = Path(directory)
        for index, (operation, text, expected) in enumerate([
            ('schema', '{"type":"object","title":"Fiktive öffentliche Struktur"}', 0),
            ('keys', '{"kty":"RSA","kid":"synthetic","n":"AQAB","e":"AQAB","alg":"unsupported"}', 2),
            ('schema', '{"$ref":"https://example.org/not-fetched"}', 3),
        ]):
            job = {'format': 'fbforge-inspect-job', 'formatVersion': 1, 'environment': 'TEST',
                   'apiVersion': 'v3', 'checks': [{'id': 'fixture', 'operation': operation, 'input': {'text': text}}]}
            source, target = root / f'job-{index}.json', root / f'result-{index}.json'
            source.write_text(json.dumps(job, ensure_ascii=False), encoding='utf-8')
            command = [java, '-Xmx256m', '-jar', str(engine), '--job', str(source), '--output', str(target)]
            result = subprocess.run(command, capture_output=True, timeout=30, env=environment)
            if result.returncode != expected:
                raise RuntimeError('cli_exitcode_mismatch:' + str(index))
            decoded = json.loads(target.read_text(encoding='utf-8'))
            if decoded['exitCode'] != expected or decoded['format'] != 'fbforge-inspect-result':
                raise RuntimeError('cli_result_contract_invalid')
            if decoded['checks'][0]['report']['requests']:
                raise RuntimeError('cli_local_fixture_started_network')
            if decoded['checks'][0]['report'].get('rawResponses'):
                raise RuntimeError('cli_raw_body_retained')
            before = target.read_bytes()
            repeat = subprocess.run(command, capture_output=True, timeout=30, env=environment)
            if repeat.returncode != 74 or target.read_bytes() != before:
                raise RuntimeError('cli_existing_result_was_not_protected')
        invalid = subprocess.run([java, '-jar', str(engine), '--unknown'], capture_output=True, timeout=30, env=environment)
        if invalid.returncode != 64:
            raise RuntimeError('cli_invalid_options_not_rejected')
    print('Portable CLI UTF-8, exit codes 0/2/3/64/74, result contract and local-only fixtures passed.')


if __name__ == '__main__':
    main()
