# App-Symbol

Die Quelle ist `inspect-source.png`. Das ausgewählte Flammenkern-Motiv verbindet
die Flamme von FBForge mit einer Lupe. Der cyanfarbene Ring ist durchgehend rund,
der helle Griff liegt dahinter. Die Schwesteranwendung FBForge Connect verwendet
Austauschpfeile mit derselben Flamme und Farbpalette.

Varianten mit der festgelegten Tauri-CLI in ein neues Verzeichnis erzeugen:

```sh
npx tauri icon assets/brand/inspect-source.png --output /tmp/inspect-brand-icons
```

Die entsprechenden Dateien nach `desktop/icons/` übernehmen. `icon.png` wird
zusätzlich nach `frontend/public/inspect.png` und `website/inspect/inspect.png`
kopiert. Anschließend die betroffenen App-Bilder aus der Oberfläche neu aufnehmen
und die Website aus dem sauberen Quellstand exportieren.

Farben: Ink `#0B1522`, Cyan `#00B8D9`, Off-White `#F4F6F8`, Flame `#F2AC32`.
