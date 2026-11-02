package de.fbforge.inspect;

import com.google.gson.*;
import java.util.*;

/**
 * Destination identity comparison; preserves every source context and never infers internal routing
 * rules.
 */
final class RoutingComparison {
  private RoutingComparison() {}

  static void inspect(Report report) {
    JsonArray routes = JsonFields.array(report.data, "routes"),
        signatures = JsonFields.array(report.data, "addressingSignatures");
    JsonArray comparisons = new JsonArray();
    report.data.add("routingComparison", comparisons);
    boolean complete =
        report.data.has("pvogRelationsComplete")
            && report.data.get("pvogRelationsComplete").getAsBoolean();
    List<JsonObject> pvog = new ArrayList<>();
    for (JsonElement element : signatures) {
      JsonObject signature = element.getAsJsonObject();
      if (JsonFields.text(signature, "context").startsWith("$.pvogRelations[")
          && qualifies(signature)) pvog.add(signature);
    }
    Set<JsonObject> used = Collections.newSetFromMap(new IdentityHashMap<>());
    for (int i = 0; i < routes.size(); i++) {
      JsonObject route = routes.get(i).getAsJsonObject();
      String destination = JsonFields.text(route, "destinationId"), path = "$.routes[" + i + "]";
      List<JsonObject> matches =
          pvog.stream().filter(s -> destination.equals(destination(s))).toList();
      used.addAll(matches);
      boolean trustedRoute =
          signatures.asList().stream()
              .filter(JsonElement::isJsonObject)
              .map(JsonElement::getAsJsonObject)
              .anyMatch(
                  s ->
                      path.equals(JsonFields.text(s, "context"))
                          && qualifies(s)
                          && destination.equals(destination(s)));
      String status =
          !trustedRoute
              ? "unknown"
              : !matches.isEmpty() ? "confirmed" : complete ? "warning" : "unknown";
      String detail =
          !trustedRoute
              ? "Routing-Adressierung für Leistung und ARS nicht vollständig verifiziert."
              : !matches.isEmpty()
                  ? "Dieselbe Destination-ID ist in Routing und im signierten"
                        + " PVOG-Leistungs-/Gebietskontext enthalten."
                  : complete
                      ? "Keine passende signierte Destination-ID in den geladenen"
                            + " PVOG-Zuständigkeiten."
                      : "PVOG-Abgleich noch nicht möglich: Auswahl, vollständige Quelle oder"
                            + " verifizierte Adressierung fehlt.";
      JsonObject row = row(destination, path, status, detail);
      JsonArray contexts = new JsonArray();
      for (JsonObject match : matches) contexts.add(JsonFields.text(match, "context"));
      row.add("pvogContexts", contexts);
      row.add("routing", route.deepCopy());
      comparisons.add(row);
    }
    for (JsonObject signature : pvog)
      if (!used.contains(signature)) {
        JsonObject row =
            row(
                destination(signature),
                "",
                "warning",
                "Signierte Destination-ID in PVOG gefunden, in dieser Routingantwort nicht"
                    + " enthalten.");
        JsonArray contexts = new JsonArray();
        contexts.add(JsonFields.text(signature, "context"));
        row.add("pvogContexts", contexts);
        comparisons.add(row);
      }
  }

  private static boolean qualifies(JsonObject signature) {
    return signature.has("verified")
        && signature.get("verified").getAsBoolean()
        && signature.has("contextMatches")
        && signature.get("contextMatches").getAsBoolean();
  }

  private static String destination(JsonObject signature) {
    return JsonFields.text(signature.getAsJsonObject("claims"), "destinationId");
  }

  private static JsonObject row(String destination, String path, String status, String detail) {
    JsonObject row = new JsonObject();
    row.addProperty("destinationId", destination);
    row.addProperty("routingPath", path);
    row.addProperty("status", status);
    row.addProperty("detail", detail);
    return row;
  }
}
