"""Observer contract only. Actual NVDA speech is tested separately on Windows."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import patch


class CaptureTests(unittest.TestCase):
    def test_new_workflow_labels_are_recorded_only_for_the_owned_window(self):
        source = Path(__file__).resolve().parents[1] / 'scripts/ci/nvda-capture.py'
        focus = SimpleNamespace(name='Hilfe durchsuchen', windowHandle=77)
        hook = SimpleNamespace(register=lambda callback: None, unregister=lambda callback: None)
        api = ModuleType('api')
        api.getFocusObject = lambda: focus
        version = ModuleType('buildVersion')
        version.version = 'fixture-observer-only'
        plugins = ModuleType('globalPluginHandler')
        plugins.GlobalPlugin = type('GlobalPlugin', (), {'terminate': lambda self: None})
        keyboard = ModuleType('inputCore')
        keyboard.decide_handleRawKey = hook
        speech = ModuleType('speech.extensions')
        speech.pre_speech = hook
        ancestor = lambda window, flags: 77
        windows = SimpleNamespace(user32=SimpleNamespace(GetAncestor=ancestor))
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'speech.jsonl'
            with patch.dict(sys.modules, {'api': api, 'buildVersion': version,
                    'globalPluginHandler': plugins, 'inputCore': keyboard,
                    'speech': ModuleType('speech'), 'speech.extensions': speech}), \
                    patch.dict(os.environ, {'INSPECT_A11Y_CAPTURE': str(output), 'INSPECT_A11Y_WINDOW': '77'}), \
                    patch('ctypes.windll', windows, create=True):
                spec = importlib.util.spec_from_file_location('capture_fixture', source)
                module = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(module)
                observer = module.GlobalPlugin()
                labels = ['Hilfe durchsuchen', 'Sammelprüfung', 'Prüfeingaben',
                          'Prüfungen vorbereiten', 'Werkzeuge', 'Aufgabe',
                          'Zertifikatsinventar', 'Serverdiagnose', 'Portables CLI exportieren',
                          'Info', 'Nach Updates suchen']
                for label in labels:
                    focus.name = label
                    observer.observe([label, object(), 'focused'])
                focus.name = 'unrelated synthetic field'
                observer.observe(['Unrelated fixture'])
                focus.windowHandle = 0
                observer.observe(['Outside owned window'])
                observer.terminate()
            records = [json.loads(line) for line in output.read_text().splitlines()]
            self.assertEqual([record['focus'] for record in records[1:-1]], labels)
            self.assertTrue(all(record['text'].endswith('focused') for record in records[1:-1]))
            self.assertEqual(records[-1]['focus'], '(other control)')
            self.assertEqual(len(records), len(labels) + 2)


if __name__ == '__main__':
    unittest.main()
