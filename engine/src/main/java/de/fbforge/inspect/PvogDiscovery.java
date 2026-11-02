package de.fbforge.inspect;

import com.google.gson.*;
import java.util.*;
import java.util.function.Function;

/** Bounded LeiKa/ARS discovery supplies suggestions, never an addressing verdict. */
final class PvogDiscovery {
  private final Report report;
  private final Function<String, JsonObject> read;

  PvogDiscovery(Report report, Function<String, JsonObject> read) {
    this.report = report;
    this.read = read;
  }

  void inspect(JsonObject input, String base, String region) {
    String service = JsonFields.value(input, "service");
    if (!service.matches("[0-9]{14}"))
      throw new IllegalArgumentException("LeiKa-Schlüssel mit 14 Ziffern erwartet.");
    String url =
        base
            + "/v3/servicedescriptions/leikaid?leikaIds="
            + service
            + "&ars="
            + region
            + "&withOrgIds=true&useArsHierarchy=true&size=10";
    JsonArray candidates = new JsonArray(), sources = new JsonArray();
    report.data.add("pvogCandidates", candidates);
    report.data.add("pvogCandidatePages", sources);
    Map<String, JsonObject> byId = new LinkedHashMap<>();
    boolean complete = false, valid = true;
    int received = 0;
    Integer expectedTotal = null;
    for (int page = 0; page < 2; page++) {
      String source = url + "&page=" + page;
      JsonObject response = read.apply(source);
      if (response == null || !response.has("content") || !response.get("content").isJsonArray())
        break;
      JsonArray rows = response.getAsJsonArray("content");
      JsonObject evidence = new JsonObject();
      evidence.addProperty("source", source);
      for (String field : List.of("pageable", "totalElements", "totalPages"))
        evidence.add(field, response.get(field));
      sources.add(evidence);
      received += rows.size();
      for (int index = 0; index < Math.min(rows.size(), 10); index++) {
        JsonElement element = rows.get(index);
        if (!element.isJsonObject()) {
          valid = false;
          continue;
        }
        JsonObject row = element.getAsJsonObject();
        String id = JsonFields.text(row, "id");
        if (id.isBlank()
            || id.length() > 255
            || id.codePoints().anyMatch(Character::isISOControl)) {
          valid = false;
          continue;
        }
        if (!JsonFields.array(row, "leikakeys").asList().stream()
            .anyMatch(
                value ->
                    value.isJsonPrimitive()
                        && value.getAsJsonPrimitive().isString()
                        && service.equals(value.getAsString()))) {
          valid = false;
          report.add(
              "error",
              "PVOG-Vorschlag passt nicht zur Leistung",
              "Eine Suchantwort enthält nicht den angefragten LeiKa-Schlüssel und wird nicht"
                  + " angeboten.",
              "$.pvogCandidates",
              source,
              "LeiKa und Quelle prüfen.",
              "Keine Zuordnung aus einem fremden Leistungskontext.");
          continue;
        }
        JsonObject candidate = byId.get(id);
        if (candidate == null) {
          candidate = new JsonObject();
          candidate.addProperty("id", id);
          candidate.addProperty("name", JsonFields.text(row, "name"));
          candidate.addProperty("source", source);
          candidate.add("organisationIds", new JsonArray());
          candidate.add("ars", new JsonArray());
          byId.put(id, candidate);
          candidates.add(candidate);
        }
        for (String field : List.of("organisationIds", "ars")) {
          JsonArray values = candidate.getAsJsonArray(field);
          for (JsonElement value : JsonFields.array(row, field))
            if (value.isJsonPrimitive()
                && value.getAsJsonPrimitive().isString()
                && value.getAsString().length() <= 255
                && !values.contains(value)) values.add(value);
        }
      }
      try {
        JsonObject pageable = response.getAsJsonObject("pageable");
        int number = JsonFields.strictCounter(pageable, "pageNumber"),
            size = JsonFields.strictCounter(pageable, "pageSize"),
            total = JsonFields.strictCounter(response, "totalElements"),
            pages = JsonFields.strictCounter(response, "totalPages");
        if (number != page
            || size != 10
            || rows.size() > size
            || total < received
            || pages != (total + (long) size - 1) / size
            || page * (long) size + rows.size() != received
            || expectedTotal != null && expectedTotal != total) break;
        expectedTotal = total;
        if (page + 1 >= pages) {
          complete = received == total && valid;
          break;
        }
        if (rows.size() != size) break;
      } catch (RuntimeException inconsistent) {
        break;
      }
    }
    report.add(
        complete ? "confirmed" : "unknown",
        "PVOG-Vorschlagssuche",
        candidates.size()
            + " Leistungsbeschreibungen zur Auswahl. Quelle: "
            + (report.environment.equals("TEST") ? "PVOG STAGE" : "PVOG PROD"),
        "$.pvogCandidates",
        url,
        "Einen Vorschlag auswählen oder eine bekannte Leistungsbeschreibungs-ID eintragen.",
        complete
            ? "Suchtreffer und OE-Verweise sind noch kein bestätigter Adressierungsvergleich."
            : "Teilbestand, Abruffehler oder inkonsistente Suchantwort. Höchstens 20 Treffer werden"
                + " geladen.");
  }
}
