# Website-Quellen

`inspect/` enthält die Projektseite für den bestehenden FBForge-Webauftritt. `npm run docs:build` erzeugt die Hilfe aus derselben Quelle wie die Desktop-App unter `dist-help/inspect/hilfe/0.1/`.

Die Projektseite verwendet die vorhandene FBForge-CSS-Datei und enthält bewusst noch keinen Installer-Download. Ein Release-Link wird erst nach tatsächlich vorhandener Veröffentlichung ergänzt. Kopieren dieser Dateien in den Website-Quellstand ist kein Deployment.

`homepage.patch` dokumentiert die Ergänzung der bestehenden Startseite um Inspect und die aktuelle Connect-Bezeichnung. Der Patch bezieht sich auf `nginx/index.html` im separaten Website-Quellstand. Vor dem Anwenden den jeweiligen Stand vergleichen. Die Projektseite nach `nginx/inspect/` und die erzeugte Hilfe nach `nginx/inspect/hilfe/0.1/` übernehmen.

Die erzeugte Hilfe enthält nun ihr eigenes `guide.css` und ein Beispielbild. Den vollständigen Ordner `dist-help/inspect/hilfe/0.1/` übernehmen; ein einzelnes HTML ohne Begleitdateien reicht nicht. Inhaltsnavigation, Typografie und Hell-/Dunkel-Farben folgen dem Connect-Handbuch. Die Seite benötigt weder JavaScript noch externe Stylesheets.
