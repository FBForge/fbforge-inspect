package de.fbforge.inspect;

import com.google.gson.*;
import java.io.IOException;
import java.util.*;

/** XZuFi 2.3.1 contexts are kept intact; identifiers and parameters never imply trust. */
final class Pvog {
  static final String DOC = "https://pvog.fitko.net/suchdienst/api/v3/api-docs";
  static final String TRANSFER_TYPE = "urn:xoev-de:fim:codeliste:xzufi.transferzustaendigkeittyp";
  static final String SIGNATURE_SCHEME = "urn:de:fitko:fit-connect:xzufi:destination";
  private final Api api;
  private final Report report;

  Pvog(Api api) {
    this.api = api;
    this.report = api.report;
  }

  void inspect(JsonObject input, Signatures signatures) throws IOException {
    String lb = Api.value(input, "lbId"), region = Api.value(input, "region").replaceFirst("^DE", "");
    if (!Api.value(input, "regionKind").equals("ars")
        || !region.matches("[0-9]{12}") || lb.isBlank() || lb.length() > 255) {
      report.unknown("PVOG-Vergleich benötigt ARS und Leistungsbeschreibungs-ID",
          "LeiKa und Leistungsbeschreibungs-ID sind unterschiedliche Kennungen. Kein Gebiet wird geraten.",
          "$", DOC);
      return;
    }
    String base = report.environment.equals("TEST")
        ? "https://stage.pvog.fitko.net/suchdienst/api"
        : "https://pvog.fitko.net/suchdienst/api";
    String url = base + "/v1/relations/jzufi-2-3?ars=" + region + "&lbids=" + Api.encode(lb)
        + "&usearshierarchy=true&size=100";
    JsonArray contexts = new JsonArray(), pages = new JsonArray();
    report.data.add("pvogRelations", contexts);
    report.data.add("pvogPages", pages);
    boolean complete = false;
    Integer expectedTotal = null, expectedPages = null;
    for (int page = 0; page < 10; page++) {
      String source = url + "&page=" + page;
      JsonObject response = read(source);
      if (response == null) break;
      // Keep the page evidence even when the metadata does not establish completeness.
      JsonObject evidence = new JsonObject();
      evidence.addProperty("source", source);
      evidence.add("page", response.get("page"));
      pages.add(evidence);
      if (!response.has("content") || !response.get("content").isJsonArray()) break;
      JsonArray content = response.getAsJsonArray("content");
      boolean valid = true;
      for (JsonElement item : content) {
        if (!item.isJsonObject() || !item.getAsJsonObject().has("zustaendigkeit")
            || !item.getAsJsonObject().get("zustaendigkeit").isJsonArray()
            || item.getAsJsonObject().getAsJsonArray("zustaendigkeit").isEmpty()) {
          valid = false;
          continue;
        }
        contexts.add(item);
        for (JsonElement relation : item.getAsJsonObject().getAsJsonArray("zustaendigkeit"))
          if (!relation.isJsonObject()) valid = false;
      }
      try {
        JsonObject metadata = response.getAsJsonObject("page");
        int number = counter(metadata, "number"), size = counter(metadata, "size"),
            total = counter(metadata, "totalElements"), totalPages = counter(metadata, "totalPages");
        if (!valid || number != page || size < 1 || content.size() > size
            || total < contexts.size() || totalPages != (total + (long) size - 1) / size
            || page * (long) size + content.size() != contexts.size()
            || expectedTotal != null && (total != expectedTotal || totalPages != expectedPages)) break;
        expectedTotal = total;
        expectedPages = totalPages;
        if (page + 1 >= totalPages) {
          complete = contexts.size() == total;
          break;
        }
        if (content.size() != size) break;
      } catch (RuntimeException invalid) {
        break;
      }
    }
    report.add(complete ? "confirmed" : "unknown", "PVOG-Quellenumfang",
        contexts.size() + " Zuständigkeitskontexte geladen. Quelle: "
            + (report.environment.equals("TEST") ? "PVOG STAGE" : "PVOG PROD"),
        "$.pvogRelations", url, "",
        complete ? "Öffentlicher Quellenvergleich, keine Rekonstruktion interner Auswahlregeln."
            : "Teilbestand oder inkonsistente Seitenmetadaten, kein vollständiger Negativnachweis.");
    String serviceSource = base + "/v1/servicedescriptions/" + pathSegment(lb) + "/jzufi-2-3";
    JsonObject service = read(serviceSource);
    if (service != null) {
      report.data.add("pvogService", service);
      report.data.addProperty("pvogServiceSource", serviceSource);
      boolean identityMatches = lb.equals(canonicalId(service.get("id"), "LB"));
      report.check(identityMatches, "PVOG-Leistungsbeschreibungszuordnung",
          "Die Antwort-ID muss zur ausdrücklich angefragten Leistungsbeschreibungs-ID passen.",
          "$.pvogService.id", serviceSource);
      boolean matched = Api.array(service, "referenzLeistungsschluessel").asList().stream()
          .filter(JsonElement::isJsonObject)
          .anyMatch(id -> text(id.getAsJsonObject(), "value").equals(Api.value(input, "service")));
      if (Api.array(service, "referenzLeistungsschluessel").isEmpty())
        report.unknown("PVOG-Leistungsschlüssel nicht veröffentlicht",
            "Die Zuordnung der Leistungsbeschreibungs-ID zum angefragten LeiKa-Schlüssel ist nicht nachgewiesen.",
            "$.pvogService.referenzLeistungsschluessel", serviceSource);
      else if (identityMatches) report.check(matched, "PVOG-Leistungszuordnung",
          "Der angefragte LeiKa-Schlüssel muss in referenzLeistungsschluessel enthalten sein.",
          "$.pvogService.referenzLeistungsschluessel", serviceSource);
    }
    JsonArray onlineServices = new JsonArray(), sources = new JsonArray();
    report.data.add("pvogOnlineServices", onlineServices);
    report.data.add("pvogOnlineServiceSources", sources);
    Map<String, Integer> cache = new HashMap<>();
    int fetched = 0;
    for (int i = 0; i < contexts.size(); i++) {
      JsonObject parent = contexts.get(i).getAsJsonObject();
      String contextPath = "$.pvogRelations[" + i + "]";
      JsonArray relations = Api.array(parent, "zustaendigkeit");
      for (int j = 0; j < relations.size(); j++) {
        if (!relations.get(j).isJsonObject()) continue;
        JsonObject relation = relations.get(j).getAsJsonObject();
        String relationPath = contextPath + ".zustaendigkeit[" + j + "]";
        JsonArray services = Api.array(relation, "leistungID");
        if (services.isEmpty())
          report.unknown("PVOG-Zuständigkeitsleistung fehlt",
              "Die Leistungsbeschreibungs-ID ist in diesem Zuständigkeitskontext nicht nachgewiesen.",
              relationPath + ".leistungID", url);
        else report.check(services.asList().stream().anyMatch(value -> lb.equals(canonicalId(value, "LB"))),
            "PVOG-Zuständigkeitsleistung",
            "Die angefragte Leistungsbeschreibungs-ID muss in derselben Zuständigkeit enthalten sein.",
            relationPath + ".leistungID", url);
        JsonArray ids = Api.array(relation, "idSekundaer");
        for (int k = 0; k < ids.size(); k++) {
          if (!ids.get(k).isJsonObject()) continue;
          JsonObject id = ids.get(k).getAsJsonObject();
          if (SIGNATURE_SCHEME.equals(text(id, "schemeID")))
            signatures.inspect(text(id, "value"), "", Api.value(input, "service"), region,
                contextPath + ".zustaendigkeit[" + j + "].idSekundaer[" + k + "]");
        }
      }
      // The authoritative class code prevents an organisation's ID being read as an OD ID.
      String id = onlineServiceId(parent);
      if (id.isEmpty()) continue;
      if (!cache.containsKey(id)) {
        if (fetched++ >= 10) {
          report.unknown("PVOG-Onlinedienstbudget erreicht",
              "Maximal zehn verschiedene Onlinedienste werden ergänzt. Weitere Kontexte bleiben erhalten.",
              contextPath, DOC);
          continue;
        }
        String source = base + "/v1/onlineservices/" + pathSegment(id) + "/jzufi-2-3";
        JsonObject detail = read(source);
        if (detail == null) {
          cache.put(id, -1);
        } else {
          JsonObject snapshot = new JsonObject();
          snapshot.addProperty("id", id);
          snapshot.addProperty("source", source);
          snapshot.add("data", detail);
          cache.put(id, sources.size());
          sources.add(snapshot);
        }
      }
      int sourceIndex = cache.get(id);
      if (sourceIndex < 0) continue;
      JsonObject detail = sources.get(sourceIndex).getAsJsonObject().getAsJsonObject("data");
      JsonObject entry = new JsonObject();
      String source = base + "/v1/onlineservices/" + pathSegment(id) + "/jzufi-2-3";
      entry.addProperty("context", contextPath);
      entry.addProperty("id", id);
      entry.addProperty("source", source);
      boolean matches = sameId(parent.get("uebergeordnetesObjektID"), detail.get("id"));
      entry.addProperty("matchesParent", matches);
      entry.addProperty("sourceIndex", sourceIndex);
      onlineServices.add(entry);
      report.check(matches, "PVOG-Onlinedienstzuordnung",
          "Wert und herausgebende Stelle der Onlinedienst-ID müssen zum übergeordneten Objekt passen.",
          contextPath + ".uebergeordnetesObjektID", source);
    }
    report.unknown("PVOG-Parameter sind Konfigurationsevidenz",
        "EfA- und individuelle Parameter bleiben ihrem Onlinedienst zugeordnet. Kommunikationskennungen und Parameter sind keine verifizierten Adressierungssignaturen und keine Fachdatenschemata.",
        "$.pvogOnlineServices", DOC);
  }

  private JsonObject read(String url) {
    try {
      return api.json(url);
    } catch (IOException failure) {
      report.unknown("PVOG-Abruf fehlgeschlagen", "Bereits geladene Kontexte bleiben als Teilbestand erhalten.", "$", url);
      return null;
    }
  }

  private static String text(JsonObject object, String key) {
    JsonElement value = object.get(key);
    return value != null && value.isJsonPrimitive() && value.getAsJsonPrimitive().isString()
        ? value.getAsString() : "";
  }

  private static int counter(JsonObject object, String key) {
    JsonElement value = object.get(key);
    if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber()
        || !value.getAsString().matches("[0-9]+")) throw new IllegalArgumentException();
    return Integer.parseInt(value.getAsString());
  }

  static String onlineServiceId(JsonObject parent) {
    if (!parent.has("uebergeordnetesObjektKlasse")
        || !parent.get("uebergeordnetesObjektKlasse").isJsonObject()
        || !parent.has("uebergeordnetesObjektID")
        || !parent.get("uebergeordnetesObjektID").isJsonObject()) return "";
    JsonObject type = parent.getAsJsonObject("uebergeordnetesObjektKlasse"),
        id = parent.getAsJsonObject("uebergeordnetesObjektID");
    if (!TRANSFER_TYPE.equals(text(type, "listURI")) || !"003".equals(text(type, "code"))) return "";
    return canonicalId(id, "OD");
  }

  private static String canonicalId(JsonElement element, String kind) {
    if (element == null || !element.isJsonObject()) return "";
    JsonObject id = element.getAsJsonObject();
    String agency = text(id, "schemeAgencyID"), value = text(id, "value");
    if (agency.isBlank() || value.isBlank()) return "";
    String result = agency + "." + kind + "." + value;
    return result.length() <= 255 ? result : "";
  }

  private static boolean sameId(JsonElement first, JsonElement second) {
    if (first == null || second == null || !first.isJsonObject() || !second.isJsonObject()) return false;
    for (String key : List.of("value", "schemeAgencyID"))
      if (text(first.getAsJsonObject(), key).isBlank()
          || !text(first.getAsJsonObject(), key).equals(text(second.getAsJsonObject(), key))) return false;
    return true;
  }

  private static String pathSegment(String value) {
    return Api.encode(value).replace("+", "%20");
  }
}
