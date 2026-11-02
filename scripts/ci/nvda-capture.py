"""CI-only NVDA speech observation for the owned, credential-free desktop window.

Loaded only from the temporary NVDA scratchpad, never bundled with the product.
The observer does not synthesize expected labels or change NVDA's speech sequence.
"""

import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path

import api
import buildVersion
import globalPluginHandler
import inputCore
from speech.extensions import pre_speech

MAX_RECORDS = 4096

class GlobalPlugin(globalPluginHandler.GlobalPlugin):
    def __init__(self):
        super().__init__()
        self.path = Path(os.environ["INSPECT_A11Y_CAPTURE"])
        self.window = int(os.environ["INSPECT_A11Y_WINDOW"])
        self.count = 0
        self.ancestor = ctypes.windll.user32.GetAncestor
        self.ancestor.argtypes = (wintypes.HWND, wintypes.UINT)
        self.ancestor.restype = wintypes.HWND
        pre_speech.register(self.observe)
        inputCore.decide_handleRawKey.register(self.observe_key)
        self.write({"ready": True, "nvdaVersion": buildVersion.version})

    def write(self, value):
        if self.count >= MAX_RECORDS:
            return
        if self.count == MAX_RECORDS - 1:
            value = {"captureLimitReached": True}
        with self.path.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(value, ensure_ascii=True) + "\n")
        self.count += 1

    def owns_focus(self):
        obj = api.getFocusObject()
        return bool(obj and obj.windowHandle and self.ancestor(obj.windowHandle, 2) == self.window)

    def observe_key(self, vkCode, scanCode, extended, pressed):
        # Observe only navigation keys in our own credential-free test window.
        if vkCode in (9, 27, 45) and self.owns_focus():
            self.write({"key": vkCode, "scan": scanCode, "extended": bool(extended), "pressed": bool(pressed)})
        return True

    def observe(self, speechSequence, **kwargs):
        if not self.owns_focus():
            return
        # Capture only NVDA-generated text for this window, never command objects.
        text = " ".join(part for part in speechSequence if isinstance(part, str))[:2048]
        if text.strip():
            name = api.getFocusObject().name
            known = {'Umgebung', 'API', 'Darstellung wählen', 'Destination prüfen', 'Destination-ID', 'Routing prüfen', 'Prozessnachricht prüfen', 'Schlüssel prüfen', 'Fachdatenschemata', 'Hilfe', 'Prüfung starten', 'JSON Schema / XSD', 'Start'}
            self.write({"text": text, "focus": name if name in known else "(other control)"})

    def terminate(self):
        pre_speech.unregister(self.observe)
        inputCore.decide_handleRawKey.unregister(self.observe_key)
        super().terminate()
