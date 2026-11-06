package de.fbforge.inspect;

import static de.fbforge.inspect.JsonFields.*;

import com.google.gson.*;
import java.io.IOException;

/** Check selected service/area tuples against one known public destination, with shared budgets. */
final class ReverseRouting {
  static void inspect(Api api, PublicHttp http, JsonObject input) throws IOException {
    Report report = api.report;
    if (report.environment.equals("STAGE"))
      throw new IllegalArgumentException(
          "Rückwärtsrouting benötigt TEST oder PROD. STAGE besitzt keine eigene Routing-API.");
    String id = uuid(value(input, "destinationId"));
    String url =
        Api.submission(report.environment) + "/" + report.apiVersion + "/destinations/" + id;
    JsonObject destination = api.json(url);
    if (destination == null) return;
    report.data.add("destination", destination);
    Api.inspectDestination(destination, input, report, url);
    JsonArray results = new JsonArray();
    report.data.add("reverseRoutes", results);
    JsonArray pairs = array(input, "pairs");
    if (pairs.size() > 12)
      throw new IllegalArgumentException(
          "Höchstens zwölf ausgewählte Routing-Kombinationen pro Lauf.");
    if (pairs.isEmpty()) {
      report.add(
          "notApplicable",
          "Routing-Kombinationen auswählen",
          "Die veröffentlichten Leistungen und Gebiete sind geladen. Eine konkrete Leistung und ein"
              + " vollständiges Suchgebiet für die Folgeprüfung auswählen.",
          "$.publicServices",
          url,
          "Rückwärtsprüfung mit ausgewählten Kombinationen starten.",
          "Kein Routingabruf ausgeführt.");
      return;
    }
    for (JsonElement element : pairs) {
      JsonObject pair = element.getAsJsonObject();
      String service = value(pair, "service"),
          region = value(pair, "region").replaceFirst("^DE", "");
      String kind = value(pair, "regionKind");
      boolean published = false;
      for (JsonElement entry : array(destination, "publicServices")) {
        JsonObject offer = entry.getAsJsonObject();
        boolean identifier =
            value(offer, "identifier").equals(service)
                || value(offer, "identifier").equals("urn:de:fim:leika:leistung:" + service);
        boolean area =
            kind.equals("areaId") || regionMatches(array(offer, "regions"), "DE" + region);
        if (identifier && area) published = true;
      }
      JsonObject row = new JsonObject();
      row.addProperty("service", service);
      row.addProperty("region", region);
      row.addProperty("regionKind", kind);
      row.addProperty("publishedContext", published && !kind.equals("areaId"));
      results.add(row);
      Report sub = new Report("routing", report.environment, report.apiVersion);
      JsonObject query = pair.deepCopy();
      query.addProperty("pvog", false);
      new Api(sub, http::get).routes(query);
      JsonArray routes = array(sub.data, "routes");
      boolean found =
          routes.asList().stream()
              .anyMatch(
                  route -> id.equalsIgnoreCase(value(route.getAsJsonObject(), "destinationId")));
      boolean complete =
          sub.findings.stream()
              .anyMatch(
                  f ->
                      f.title().equals("Ergebnisumfang")
                          && f.path().equals("$.routes")
                          && f.status().equals("confirmed"));
      row.addProperty("complete", complete);
      row.addProperty("expectedFound", found);
      row.add("routes", routes);
      row.add("findings", Main.JSON.toJsonTree(sub.findings));
      String status = !complete ? "unknown" : found ? "confirmed" : "warning";
      report.add(
          status,
          "Rückwärtsrouting · " + service,
          found
              ? "Der erwartete Zustellpunkt wird zurückgegeben. Weitere Zuständigkeiten bleiben"
                    + " sichtbar."
              : complete
                  ? "Der erwartete Zustellpunkt wurde für diese Abfrage nicht zurückgegeben."
                  : "Die Abfrage ist unvollständig. Kein vollständiger Negativnachweis möglich.",
          "$.reverseRoutes[" + (results.size() - 1) + "]",
          Api.routing(report.environment),
          found
              ? ""
              : "Leistungs-/Gebietskontext und veröffentlichte Adressierungsinformationen gezielt"
                    + " prüfen.",
          "Öffentliche Auffindbarkeit ist keine authentifizierte Zustellprobe. Die interne Ursache"
              + " einer fehlenden Route ist damit nicht bewiesen.");
    }
  }
}
