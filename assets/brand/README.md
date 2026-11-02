# App-Symbol

Die Quelle ist `inspect-source.png`. Das ausgewählte Flammenkern-Motiv verbindet
die Flamme von FBForge mit einer Lupe. Der cyanfarbene Ring ist durchgehend rund,
der helle Griff liegt dahinter. Die Schwesteranwendung FBForge Connect verwendet
Austauschpfeile mit derselben Flamme und Farbpalette.

Varianten mit der festgelegten Tauri-CLI in ein neues Verzeichnis erzeugen:

```sh
npx tauri icon assets/brand/inspect-source.png --output /tmp/inspect-brand-icons
```

Für Windows x64 nur `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.ico`
und `icon.png` nach `desktop/icons/` übernehmen. Weitere Plattform- und
Größenexporte bleiben im temporären Verzeichnis. Die README verwendet die
vorhandene 256-Pixel-Variante. `icon.png` wird
zusätzlich nach `frontend/public/inspect.png` und `website/inspect/inspect.png`
kopiert. Anschließend die betroffenen App-Bilder aus der Oberfläche neu aufnehmen
und die Website aus dem sauberen Quellstand exportieren.

Farben: Ink `#0B1522`, Cyan `#00B8D9`, Off-White `#F4F6F8`, Flame `#F2AC32`.
