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
    String lb = JsonFields.value(input, "lbId"),
        region = JsonFields.value(input, "region").replaceFirst("^DE", "");
    if (!JsonFields.value(input, "regionKind").equals("ars")
        || !region.matches("[0-9]{12}")
        || lb.length() > 255) {
      report.unknown(
          FindingCode.PVOG_INPUT_REQUIRED,
          "PVOG-Vergleich benötigt gültige Gebiets- und Leistungsangaben",
          "Ein 12-stelliger ARS ist erforderlich. Eine bekannte Leistungsbeschreibungs-ID darf"
              + " höchstens 255 Zeichen enthalten. Ohne ID werden Vorschläge gesucht.",
          "$",
          DOC);
      return;
    }
    String base =
        report.environment.equals("TEST")
            ? "https://stage.pvog.fitko.net/suchdienst/api"
            : "https://pvog.fitko.net/suchdienst/api";
    if (lb.isBlank()) {
      lb = new PvogDiscovery(report, this::read).inspect(input, base, region);
      if (lb.isBlank()) return;
      report.data.addProperty("pvogAutomaticallySelected", true);
    }
    report.data.addProperty("pvogSelectedLbId", lb);
    JsonArray contexts = loadRelations(base, region, lb);
    inspectService(base, lb, input);
    inspectContexts(base, contexts, input, region, lb, signatures);
  }

  private JsonArray loadRelations(String base, String region, String lb) {
    String url =
        base
            + "/v1/relations/jzufi-2-3?ars="
            + region
            + "&lbids="
            + JsonFields.encode(lb)
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
        if (!item.isJsonObject()
            || !item.getAsJsonObject().has("zustaendigkeit")
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
        int number = JsonFields.strictCounter(metadata, "number"),
            size = JsonFields.strictCounter(metadata, "size"),
            total = JsonFields.strictCounter(metadata, "totalElements"),
            totalPages = JsonFields.strictCounter(metadata, "totalPages");
        if (!valid
            || number != page
            || size < 1
            || content.size() > size
            || total < contexts.size()
            || totalPages != (total + (long) size - 1) / size
            || page * (long) size + content.size() != contexts.size()
            || expectedTotal != null && (total != expectedTotal || totalPages != expectedPages))
          break;
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
    report.add(
        complete ? "confirmed" : "unknown",
        "PVOG-Quellenumfang",
        contexts.size()
            + " Zuständigkeitskontexte geladen. Quelle: "
            + (report.environment.equals("TEST") ? "PVOG STAGE" : "PVOG PROD"),
        "$.pvogRelations",
        url,
        "",
        complete
            ? "Öffentlicher Quellenvergleich, keine Rekonstruktion interner Auswahlregeln."
            : "Teilbestand oder inkonsistente Seitenmetadaten, kein vollständiger"
                + " Negativnachweis.");
    report.data.addProperty("pvogRelationsComplete", complete);
    return contexts;
  }

  private void inspectService(String base, String lb, JsonObject input) {
    String serviceSource =
        base + "/v1/servicedescriptions/" + JsonFields.pathSegment(lb) + "/jzufi-2-3";
    JsonObject service = read(serviceSource);
    if (service != null) {
      report.data.add("pvogService", service);
      report.data.addProperty("pvogServiceSource", serviceSource);
      boolean identityMatches = lb.equals(canonicalId(service.get("id"), "LB"));
      report.check(
          identityMatches,
          "PVOG-Leistungsbeschreibungszuordnung",
          "Die Antwort-ID muss zur ausdrücklich angefragten Leistungsbeschreibungs-ID passen.",
          "$.pvogService.id",
          serviceSource);
      boolean matched =
          JsonFields.array(service, "referenzLeistungsschluessel").asList().stream()
              .filter(JsonElement::isJsonObject)
              .anyMatch(
                  id ->
                      JsonFields.text(id.getAsJsonObject(), "value")
                          .equals(JsonFields.value(input, "service")));
      if (JsonFields.array(service, "referenzLeistungsschluessel").isEmpty())
        report.unknown(
            "PVOG-Leistungsschlüssel nicht veröffentlicht",
            "Die Zuordnung der Leistungsbeschreibungs-ID zum angefragten LeiKa-Schlüssel ist nicht"
                + " nachgewiesen.",
            "$.pvogService.referenzLeistungsschluessel",
            serviceSource);
      else if (identityMatches)
        report.check(
            matched,
            "PVOG-Leistungszuordnung",
            "Der angefragte LeiKa-Schlüssel muss in referenzLeistungsschluessel enthalten sein.",
            "$.pvogService.referenzLeistungsschluessel",
            serviceSource);
    }
  }

  private void inspectContexts(
      String base,
      JsonArray contexts,
      JsonObject input,
      String region,
      String lb,
      Signatures signatures) {
    String url =
        base
            + "/v1/relations/jzufi-2-3?ars="
            + region
            + "&lbids="
            + JsonFields.encode(lb)
            + "&usearshierarchy=true&size=100";
    JsonArray onlineServices = new JsonArray(), sources = new JsonArray();
    report.data.add("pvogOnlineServices", onlineServices);
    report.data.add("pvogOnlineServiceSources", sources);
    Map<String, Integer> cache = new HashMap<>();
    int fetched = 0;
    for (int i = 0; i < contexts.size(); i++) {
      JsonObject parent = contexts.get(i).getAsJsonObject();
      String contextPath = "$.pvogRelations[" + i + "]";
      JsonArray relations = JsonFields.array(parent, "zustaendigkeit");
      for (int j = 0; j < relations.size(); j++) {
        if (!relations.get(j).isJsonObject()) continue;
        JsonObject relation = relations.get(j).getAsJsonObject();
        String relationPath = contextPath + ".zustaendigkeit[" + j + "]";
        JsonArray services = JsonFields.array(relation, "leistungID");
        if (services.isEmpty())
          report.unknown(
              "PVOG-Zuständigkeitsleistung fehlt",
              "Die Leistungsbeschreibungs-ID ist in diesem Zuständigkeitskontext nicht"
                  + " nachgewiesen.",
              relationPath + ".leistungID",
              url);
        else
          report.check(
              services.asList().stream().anyMatch(value -> lb.equals(canonicalId(value, "LB"))),
              "PVOG-Zuständigkeitsleistung",
              "Die angefragte Leistungsbeschreibungs-ID muss in derselben Zuständigkeit enthalten"
                  + " sein.",
              relationPath + ".leistungID",
              url);
        inspectCommunication(relation, input, region, relationPath, signatures);
        JsonArray ids = JsonFields.array(relation, "idSekundaer");
        for (int k = 0; k < ids.size(); k++) {
          if (!ids.get(k).isJsonObject()) continue;
          JsonObject id = ids.get(k).getAsJsonObject();
          if (SIGNATURE_SCHEME.equals(JsonFields.text(id, "schemeID")))
            signatures.inspect(
                JsonFields.text(id, "value"),
                "",
                JsonFields.value(input, "service"),
                region,
                contextPath + ".zustaendigkeit[" + j + "].idSekundaer[" + k + "]");
        }
      }
      // The authoritative class code prevents an organisation's ID being read as an OD ID.
      String id = onlineServiceId(parent);
      if (id.isEmpty()) continue;
      if (!cache.containsKey(id)) {
        if (fetched++ >= 10) {
          report.unknown(
              "PVOG-Onlinedienstbudget erreicht",
              "Maximal zehn verschiedene Onlinedienste werden ergänzt. Weitere Kontexte bleiben"
                  + " erhalten.",
              contextPath,
              DOC);
          continue;
        }
        String source = base + "/v1/onlineservices/" + JsonFields.pathSegment(id) + "/jzufi-2-3";
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
      String source = base + "/v1/onlineservices/" + JsonFields.pathSegment(id) + "/jzufi-2-3";
      entry.addProperty("context", contextPath);
      entry.addProperty("id", id);
      entry.addProperty("source", source);
      boolean matches = sameId(parent.get("uebergeordnetesObjektID"), detail.get("id"));
      entry.addProperty("matchesParent", matches);
      entry.addProperty("sourceIndex", sourceIndex);
      onlineServices.add(entry);
      report.check(
          matches,
          "PVOG-Onlinedienstzuordnung",
          "Wert und herausgebende Stelle der Onlinedienst-ID müssen zum übergeordneten Objekt"
              + " passen.",
          contextPath + ".uebergeordnetesObjektID",
          source);
    }
  }

  private void inspectCommunication(
      JsonObject relation, JsonObject input, String region, String path, Signatures signatures) {
    JsonArray systems = JsonFields.array(relation, "kommunikationssystem");
    for (int i = 0; i < systems.size(); i++) {
      if (!systems.get(i).isJsonObject()) continue;
      JsonObject system = systems.get(i).getAsJsonObject();
      if (!system.has("kanal") || !system.get("kanal").isJsonObject()) continue;
      JsonObject channel = system.getAsJsonObject("kanal");
      if (!"004".equals(JsonFields.text(channel, "code"))
          || !"urn:xoev-de:fim:codeliste:xzufi.kommunikationssystemtyp"
              .equals(JsonFields.text(channel, "listURI"))) continue;
      // The channel identifies the format, not trust. The same independent JWKS/claim checks apply.
      signatures.inspect(
          JsonFields.text(system, "kennung"),
          "",
          JsonFields.value(input, "service"),
          region,
          path + ".kommunikationssystem[" + i + "].kennung");
    }
  }

  private JsonObject read(String url) {
    try {
      return api.json(url);
    } catch (IOException failure) {
      report.unknown(
          "PVOG-Abruf fehlgeschlagen",
          "Bereits geladene Kontexte bleiben als Teilbestand erhalten.",
          "$",
          url);
      return null;
    }
  }

  static String onlineServiceId(JsonObject parent) {
    if (!parent.has("uebergeordnetesObjektKlasse")
        || !parent.get("uebergeordnetesObjektKlasse").isJsonObject()
        || !parent.has("uebergeordnetesObjektID")
        || !parent.get("uebergeordnetesObjektID").isJsonObject()) return "";
    JsonObject type = parent.getAsJsonObject("uebergeordnetesObjektKlasse"),
        id = parent.getAsJsonObject("uebergeordnetesObjektID");
    if (!TRANSFER_TYPE.equals(JsonFields.text(type, "listURI"))
        || !"003".equals(JsonFields.text(type, "code"))) return "";
    return canonicalId(id, "OD");
  }

  private static String canonicalId(JsonElement element, String kind) {
    if (element == null || !element.isJsonObject()) return "";
    JsonObject id = element.getAsJsonObject();
    String agency = JsonFields.text(id, "schemeAgencyID"), value = JsonFields.text(id, "value");
    if (agency.isBlank() || value.isBlank()) return "";
    String result = agency + "." + kind + "." + value;
    return result.length() <= 255 ? result : "";
  }

  private static boolean sameId(JsonElement first, JsonElement second) {
    if (first == null || second == null || !first.isJsonObject() || !second.isJsonObject())
      return false;
    for (String key : List.of("value", "schemeAgencyID"))
      if (JsonFields.text(first.getAsJsonObject(), key).isBlank()
          || !JsonFields.text(first.getAsJsonObject(), key)
              .equals(JsonFields.text(second.getAsJsonObject(), key))) return false;
    return true;
  }
}
