package de.fbforge.inspect;

import com.google.gson.*;
import java.io.*;
import java.net.*;
import java.util.*;

/** Read-only probes of required FIT-Connect services and selected certificate endpoints. */
final class NetworkDiagnostics {
  private NetworkDiagnostics() {}

  record Target(String name, String url, byte[] post, boolean pki) {}

  static void inspect(JsonObject input, Report report, PublicHttp http) throws Exception {
    JsonArray results = new JsonArray();
    report.data.add("networkChecks", results);
    List<Target> targets = new ArrayList<>();
    String submission = URI.create(Api.submission(report.environment)).getHost();
    targets.add(
        new Target(
            "FIT-Connect · öffentliche Schlüssel",
            "https://" + submission + "/.well-known/jwks.json",
            null,
            false));
    targets.add(
        new Target(
            report.environment.equals("STAGE")
                ? "Routing API v2 · PROD (für STAGE)"
                : "Routing API v2",
            Api.routing(report.environment)
                + "/v2/areas?areaSearchexpression=Berlin&limit=1&offset=0",
            null,
            false));
    targets.addAll(CertificateEndpoints.probes(input));
    if (input.has("certificates"))
      report.data.add("networkCertificates", input.get("certificates").deepCopy());
    JsonArray extra = JsonFields.array(input, "certificateEndpoints");
    if (extra.size() > 12)
      throw new IllegalArgumentException(
          "Höchstens zwölf Zertifikatsendpunkte pro Netzwerkprüfung.");
    int index = 0;
    for (JsonElement element : extra) {
      if (!element.isJsonPrimitive() || !element.getAsJsonPrimitive().isString())
        throw new IllegalArgumentException("Öffentliche Zertifikatsendpunkte als URLs angeben.");
      String url = element.getAsString();
      PublicHttp.validateUri(URI.create(url), true);
      if (targets.stream().noneMatch(target -> target.url().equals(url)))
        targets.add(new Target("Zertifikatsendpunkt " + (++index), url, null, true));
    }
    if (targets.size() > 16)
      throw new IllegalArgumentException(
          "Höchstens sechzehn unterschiedliche Ziele pro Netzwerkprüfung. Weniger Zertifikate"
              + " auswählen.");
    String onlyUrl = JsonFields.value(input, "onlyUrl");
    if (!onlyUrl.isBlank()) {
      targets.removeIf(target -> !target.url().equals(onlyUrl));
      if (targets.isEmpty())
        throw new IllegalArgumentException(
            "Ausgewähltes Netzwerkziel ist nicht Bestandteil der vorbereiteten Prüfung.");
    }
    for (var target : targets) {
      JsonObject item = new JsonObject();
      String url = target.url();
      URI uri = URI.create(url);
      item.addProperty("name", target.name());
      item.addProperty("url", url);
      item.addProperty("host", uri.getHost());
      item.addProperty("port", uri.getScheme().equals("https") ? 443 : 80);
      item.addProperty("method", target.post() == null ? "GET" : "POST");
      item.addProperty("certificateEndpoint", target.pki());
      results.add(item);
      long started = System.nanoTime();
      try {
        PublicHttp.Response response = target.pki() ? http.pki(url, target.post()) : http.get(url);
        boolean unexpected =
            !target.pki()
                && response.ok()
                && (response
                        .headers()
                        .getOrDefault("content-type", "")
                        .toLowerCase(Locale.ROOT)
                        .contains("text/html")
                    || response.text().stripLeading().startsWith("<"));
        item.addProperty("httpStatus", response.status());
        item.addProperty("status", response.ok() && !unexpected ? "confirmed" : "warning");
        if (unexpected) item.addProperty("code", "HTTP_UNEXPECTED_CONTENT");
        item.addProperty(
            "detail",
            unexpected
                ? "HTTP-Antwort erhalten, aber HTML statt der erwarteten öffentlichen JSON-Daten."
                      + " Eine vorgeschaltete Sperr- oder Anmeldeseite ist möglich."
                : response.ok()
                    ? "DNS, Verbindung und HTTP-Abruf erfolgreich."
                    : "HTTP "
                        + response.status()
                        + ": Der Dienst antwortet. Der konkrete Abruf war nicht erfolgreich.");
        report.add(
            response.ok() && !unexpected ? "confirmed" : "warning",
            target.name(),
            item.get("detail").getAsString(),
            "$.networkChecks[" + (results.size() - 1) + "]",
            url,
            "",
            target.pki()
                ? "Netzwerkerreichbarkeit. Signatur und Sperrstatus werden in der"
                    + " Zertifikatsprüfung separat ausgewertet."
                : "Dieser öffentliche Abruf bestätigt nicht sämtliche Funktionen des Dienstes.");
      } catch (IOException failure) {
        String code = failure instanceof NetworkFailure known ? known.code() : "NETWORK_FAILED";
        item.addProperty("code", code);
        item.addProperty("status", "unknown");
        item.addProperty(
            "detail",
            failure instanceof NetworkFailure known
                ? known.description()
                : "Verbindung oder Antwort nicht auswertbar.");
        report.unknown(
            target.name(),
            item.get("detail").getAsString(),
            "$.networkChecks[" + (results.size() - 1) + "]",
            url);
      } finally {
        item.addProperty("durationMs", (System.nanoTime() - started) / 1_000_000);
      }
    }
    // Network checks export connection evidence, not complete API or CRL response bodies.
    report.rawResponses.clear();
  }
}
