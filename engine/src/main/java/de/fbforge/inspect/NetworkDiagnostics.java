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
        item.addProperty("httpStatus", response.status());
        item.addProperty("status", response.ok() ? "confirmed" : "warning");
        item.addProperty(
            "detail",
            response.ok()
                ? "DNS, Verbindung und HTTP-Abruf erfolgreich."
                : "HTTP "
                    + response.status()
                    + ": Der Dienst antwortet. Der konkrete Abruf war nicht erfolgreich.");
        report.add(
            response.ok() ? "confirmed" : "warning",
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
