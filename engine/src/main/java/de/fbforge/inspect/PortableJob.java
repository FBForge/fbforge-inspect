package de.fbforge.inspect;

import com.google.gson.*;
import java.util.*;

/** Public, portable check inputs. Credentials are deliberately not part of this format. */
final class PortableJob {
  static final int MAX_CHECKS = 32;
  static final Set<String> OPERATIONS =
      Set.of(
          "network",
          "destination",
          "routing",
          "reverse",
          "areas",
          "process",
          "limits",
          "keys",
          "certificate",
          "signature",
          "schema",
          "rollover");

  static List<JsonObject> requests(JsonObject job) {
    JsonBounds.check(job);
    if (!Set.of("format", "formatVersion", "environment", "apiVersion", "checks", "network")
        .containsAll(job.keySet()))
      throw new IllegalArgumentException("Unbekannte Felder im Prüfauftrag.");
    if (!JsonFields.value(job, "format").equals("fbforge-inspect-job")
        || JsonFields.strictCounter(job, "formatVersion") != 1)
      throw new IllegalArgumentException("Prüfauftrag oder Formatversion ungültig.");
    String environment = JsonFields.value(job, "environment");
    Api.submission(environment);
    String version = JsonFields.value(job, "apiVersion");
    if (!Set.of("v2", "v3").contains(version))
      throw new IllegalArgumentException("API-Version muss v2 oder v3 sein.");
    if (KeyDiagnostics.hasPrivateMaterial(job.toString()))
      throw new IllegalArgumentException("Prüfaufträge dürfen nur öffentliche Daten enthalten.");
    JsonObject network = job.has("network") ? job.getAsJsonObject("network") : new JsonObject();
    for (String field : network.keySet())
      if (!Set.of("mode", "proxyUrl", "httpsCaPem").contains(field))
        throw new IllegalArgumentException("Zugangsdaten gehören nicht in den Prüfauftrag.");
    if (network.size() > 0) NetworkOptions.read(network);
    JsonArray checks = JsonFields.array(job, "checks");
    if (checks.isEmpty() || checks.size() > MAX_CHECKS)
      throw new IllegalArgumentException("Zwischen 1 und 32 Prüfungen erwartet.");
    List<JsonObject> requests = new ArrayList<>();
    Set<String> ids = new HashSet<>();
    for (JsonElement element : checks) {
      JsonObject check = element.getAsJsonObject();
      if (!Set.of("id", "operation", "input").containsAll(check.keySet()))
        throw new IllegalArgumentException("Unbekannte Felder in der Prüfung.");
      String id = JsonFields.value(check, "id"), operation = JsonFields.value(check, "operation");
      if (!id.matches("[A-Za-z0-9_-]{1,64}") || !ids.add(id) || !OPERATIONS.contains(operation))
        throw new IllegalArgumentException("Prüfkennung oder Diagnosefunktion ungültig.");
      if (!check.has("input") || !check.get("input").isJsonObject())
        throw new IllegalArgumentException("Prüfeingabe fehlt.");
      if (!Set.of(
              "destinationId",
              "service",
              "region",
              "regionKind",
              "schema",
              "mime",
              "metadata",
              "compare",
              "revocation",
              "trustPem",
              "processModelId",
              "messageId",
              "fromDestinationId",
              "toDestinationId",
              "text",
              "url",
              "areaSearch",
              "pvog",
              "lbId",
              "pairs",
              "candidateKid",
              "certificates",
              "certificateEndpoints",
              "onlyUrl")
          .containsAll(check.getAsJsonObject("input").keySet()))
        throw new IllegalArgumentException("Unbekannte oder vertrauliche Prüfeingabe.");
      JsonObject input = check.getAsJsonObject("input");
      if (KeyDiagnostics.hasPrivateMaterial(
              JsonFields.value(input, "text"), operation.equals("keys"))
          || KeyDiagnostics.hasPrivateMaterial(JsonFields.value(input, "trustPem"), true))
        throw new IllegalArgumentException("Private Schlüssel gehören nicht in Prüfaufträge.");
      for (JsonElement pair : JsonFields.array(input, "pairs"))
        if (!Set.of("service", "region", "regionKind").containsAll(pair.getAsJsonObject().keySet()))
          throw new IllegalArgumentException("Unbekannte Routing-Kombination.");
      String url = JsonFields.value(input, "url");
      if (!url.isBlank()) PublicHttp.validateUri(java.net.URI.create(url), false);
      JsonObject request = new JsonObject();
      request.addProperty("protocolVersion", 1);
      request.addProperty("operation", operation);
      request.addProperty("environment", environment);
      request.addProperty("apiVersion", version);
      request.add("input", check.getAsJsonObject("input").deepCopy());
      if (network.size() > 0) request.add("network", network.deepCopy());
      requests.add(request);
    }
    return requests;
  }
}
