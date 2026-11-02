package de.fbforge.inspect;

import static de.fbforge.inspect.JsonFields.*;

import com.google.gson.*;
import java.io.IOException;
import java.util.*;

/** Shared public JSON retrieval and pagination; partial responses remain explicit. */
final class PublicJsonClient {
  private final Report report;
  private final Api.Fetch fetch;

  PublicJsonClient(Report report, Api.Fetch fetch) {
    this.report = report;
    this.fetch = fetch;
  }

  public JsonObject json(String url) throws IOException {
    PublicHttp.Response response = fetch.get(url);
    if (!response.ok()) {
      String detail =
          "HTTP "
              + response.status()
              + ". "
              + switch (response.status()) {
                case 401 ->
                    "Die Quelle hat den Zugriff zurückgewiesen. Eine öffentliche Antwort wurde"
                        + " nicht geliefert.";
                case 403 -> "Die Quelle erlaubt diesen Abruf nicht.";
                case 404 -> "Unter dieser Adresse wurde kein Eintrag gefunden.";
                case 429 -> "Das Ratenlimit der Quelle ist erreicht.";
                default ->
                    response.status() >= 500
                        ? "Der Dienst meldet eine Störung."
                        : "Die Quelle hat den Abruf nicht erfolgreich beantwortet.";
              };
      String next =
          switch (response.status()) {
            case 401, 403 ->
                "Umgebung und öffentliche Quelle prüfen. Ein Zugriffsfehler ist kein leerer"
                    + " Suchbestand.";
            case 404 ->
                "Kennung und Umgebung prüfen. Ein nicht gefundener Eintrag ist keine Prüfung seiner"
                    + " Konfiguration.";
            case 429 -> "Später erneut versuchen. Kein automatischer Wiederholungsabruf.";
            default ->
                "Die betroffene Quelle später erneut prüfen. Bereits geladene Daten sind nur ein"
                    + " Teilbestand.";
          };
      report.add(
          "unknown",
          "Abruf nicht erfolgreich",
          detail,
          "$",
          url,
          next,
          "Keine erfolgreiche öffentliche Antwort ausgewertet.");
      return null;
    }
    try {
      if (KeyDiagnostics.hasPrivateMaterial(response.text()))
        throw new IllegalArgumentException("Nicht öffentliche Schlüsselantwort.");
      return JsonParser.parseString(response.text()).getAsJsonObject();
    } catch (Exception e) {
      report.unknown(
          "Antwortformat nicht lesbar", "Öffentliche JSON-Objektantwort erwartet.", "$", url);
      return null;
    }
  }

  public JsonArray pages(String url, String field) throws IOException {
    JsonArray result = new JsonArray();
    boolean complete = false;
    for (int offset = 0, page = 0; page < 20; page++) {
      JsonObject response;
      try {
        response = json(url + (url.contains("?") ? "&" : "?") + "limit=100&offset=" + offset);
      } catch (IOException e) {
        report.unknown(
            "Seitenabruf fehlgeschlagen", "Bereits geladene Kontexte bleiben erhalten.", "$", url);
        break;
      }
      if (response == null) break;
      JsonArray items = array(response, field);
      int total;
      try {
        total = counter(response, "totalCount");
        if (counter(response, "offset") != offset
            || counter(response, "count") != items.size()
            || items.size() > 500
            || total < (long) offset + items.size()
            || !response.has(field)
            || !response.get(field).isJsonArray()) throw new IllegalArgumentException();
      } catch (RuntimeException invalid) {
        report.unknown(
            "Unvollständige Paginierung",
            "Zähler oder Offset widersprechen der Antwort.",
            "$",
            url);
        break;
      }
      result.addAll(items);
      offset += items.size();
      if (offset >= total) {
        complete = true;
        break;
      }
      if (items.isEmpty()) break;
    }
    report.add(
        complete ? "confirmed" : "unknown",
        "Ergebnisumfang",
        result.size()
            + " Kontexte geladen. "
            + (complete
                ? "Alle gemeldeten Seiten abgerufen."
                : "Nur ein Teilbestand ist nachgewiesen."),
        "$." + field,
        url,
        "",
        complete ? "" : "Kein vollständiger Negativnachweis möglich.");
    return result;
  }
}
