package de.fbforge.inspect;

import static de.fbforge.inspect.Api.routing;
import static de.fbforge.inspect.JsonFields.*;

import com.google.gson.*;
import java.io.IOException;
import java.util.*;

/** Routing and area discovery keep their original public contexts. */
final class RoutingDiagnostics {
  private final Api api;
  private final Report report;

  RoutingDiagnostics(Api api) {
    this.api = api;
    this.report = api.report;
  }

  public void routes(JsonObject input) throws IOException {
    String service = value(input, "service"),
        region = value(input, "region").replaceFirst("^DE", "");
    if (!service.matches("[0-9]{14}"))
      throw new IllegalArgumentException("LeiKa-Schlüssel mit 14 Ziffern erwartet.");
    String regionKind = value(input, "regionKind"),
        parameter =
            switch (regionKind) {
              case "ags" -> {
                if (!region.matches("[0-9]{8}"))
                  throw new IllegalArgumentException("AGS mit 8 Ziffern erwartet.");
                yield "ags";
              }
              case "ars" -> {
                if (!region.matches("[0-9]{12}"))
                  throw new IllegalArgumentException("ARS mit 12 Ziffern erwartet.");
                yield "ars";
              }
              case "areaId" -> {
                if (!region.matches("[0-9]{1,20}"))
                  throw new IllegalArgumentException("Gebiets-ID erwartet.");
                yield "areaId";
              }
              default -> throw new IllegalArgumentException("Gebietsart fehlt.");
            };
    String url =
        routing(report.environment)
            + "/v2/routes?leikaKey="
            + service
            + "&"
            + parameter
            + "="
            + region;
    JsonArray routes = api.pages(url, "routes");
    report.data.add("routes", routes);
    Signatures signatures = new Signatures(api);
    int i = 0;
    for (JsonElement route : routes) {
      JsonObject row = route.getAsJsonObject();
      String signature = value(row, "destinationSignature");
      if (!signature.isBlank())
        signatures.inspect(
            signature,
            value(row, "destinationId"),
            service,
            parameter.equals("ars") ? region : "",
            "$.routes[" + (i++) + "]");
      else
        report.unknown(
            "Adressierungssignatur fehlt",
            "Keine destinationSignature im öffentlichen Routenkontext.",
            "$.routes[" + (i++) + "]",
            url);
    }
    if (!input.has("pvog") || input.get("pvog").getAsBoolean()) {
      new Pvog(api).inspect(input, signatures);
      RoutingComparison.inspect(report);
    }
    if (report.environment.equals("STAGE"))
      report.add(
          "warning",
          "Routingquelle PROD",
          "STAGE hat keinen eigenen Routingdienst. Angezeigt werden PROD-Routen, keine"
              + " STAGE-Zustellbarkeit.",
          "$",
          Api.DOC,
          "STAGE-Destination direkt prüfen.",
          "");
  }

  public void areas(JsonObject input) throws IOException {
    String search = value(input, "areaSearch");
    if (search.length() < 2 || search.length() > 150)
      throw new IllegalArgumentException("Gebietssuche mit 2 bis 150 Zeichen erwartet.");
    report.data.add(
        "areas",
        api.pages(
            routing(report.environment) + "/v2/areas?areaSearchexpression=" + encode(search),
            "areas"));
  }
}
