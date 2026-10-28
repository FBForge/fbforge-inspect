package de.fbforge.inspect;

import com.google.gson.*;
import java.io.IOException;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Implements only the documented public read surface, independently of any SDK. */
public final class Api {
  public static final String DOC =
      "https://docs.fitko.de/fit-connect/docs/sending/get-destination/";

  public interface Fetch {
    PublicHttp.Response get(String url) throws IOException;
  }

  final Report report;
  final Fetch fetch;

  public Api(Report report, Fetch fetch) {
    this.report = report;
    this.fetch = fetch;
  }

  public static String submission(String env) {
    return switch (env) {
      case "TEST" -> "https://test.fit-connect.fitko.dev/submission-api";
      case "STAGE" -> "https://stage.fit-connect.fitko.net/submission-api";
      case "PROD" -> "https://prod.fit-connect.fitko.net/submission-api";
      default -> throw new IllegalArgumentException("Unbekannte Umgebung.");
    };
  }

  public static String routing(String env) {
    return env.equals("TEST")
        ? "https://routing-api-testing.fit-connect.fitko.dev"
        : "https://routing-api-prod.fit-connect.fitko.net";
  }

  public static String value(JsonObject o, String key) {
    return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : "";
  }

  public static String encode(String value) {
    return URLEncoder.encode(value, StandardCharsets.UTF_8);
  }

  public static String uuid(String value) {
    if (!value.matches(
        "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}"))
      throw new IllegalArgumentException("Destination-ID muss eine UUID sein.");
    return value;
  }

  public JsonObject json(String url) throws IOException {
    PublicHttp.Response response = fetch.get(url);
    if (!response.ok()) {
      String detail =
          "HTTP "
              + response.status()
              + ". "
              + (response.status() == 429
                  ? "Ratenlimit erreicht. RateLimit-Reset: "
                      + response.headers().getOrDefault("ratelimit-reset", "unbekannt")
                  : "Quelle hat keine erfolgreiche Antwort geliefert.");
      report.unknown("Abruf nicht erfolgreich", detail, "$", url);
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

  public JsonObject destination(JsonObject input, PublicHttp http) throws IOException {
    String id = uuid(value(input, "destinationId")),
        url = submission(report.environment) + "/" + report.apiVersion + "/destinations/" + id;
    JsonObject d = json(url);
    if (d == null) return null;
    report.data.add("destination", d);
    inspectDestination(d, input, report, url);
    String kid = value(d, "encryptionKid");
    if (!kid.isBlank()) {
      String keyUrl = url + "/keys/" + encode(kid);
      JsonObject key = json(keyUrl);
      if (key != null) KeyDiagnostics.inspect(key.toString(), report, http, input);
    } else
      report.add(
          value(d, "type").equals("C") ? "notApplicable" : "unknown",
          "Verschlüsselungsschlüssel",
          "Kein encryptionKid veröffentlicht.",
          "$.encryptionKid",
          url,
          "",
          "Rolle des Zustellpunkts beachten.");
    if (input.has("compare") && input.get("compare").getAsBoolean()) {
      String other = report.apiVersion.equals("v3") ? "v2" : "v3";
      JsonObject c = json(submission(report.environment) + "/" + other + "/destinations/" + id);
      if (c != null) report.data.add("comparison" + other, c);
      report.add(
          "notApplicable",
          "Versionsvergleich",
          "Fehlende v3-Felder in v2 sind kein Konfigurationsfehler.",
          "$",
          url,
          "",
          "");
    }
    return d;
  }

  public static void inspectDestination(JsonObject d, JsonObject input, Report r, String url) {
    if (!value(input, "destinationId").isBlank())
      r.check(
          value(input, "destinationId").equalsIgnoreCase(value(d, "destinationId")),
          "Destination-ID",
          "Antwort-ID muss der angefragten Destination entsprechen.",
          "$.destinationId",
          url);
    r.check(
        value(d, "status").equals("active"),
        "Destination-Status",
        value(d, "status"),
        "$.status",
        url);
    JsonArray versions = array(d, "metadataVersions");
    boolean validVersions = !versions.isEmpty();
    for (JsonElement version : versions)
      if (!version.isJsonPrimitive()
          || !version.getAsJsonPrimitive().isString()
          || !version.getAsString().matches("(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)"))
        validVersions = false;
    r.check(
        validVersions,
        "Metadaten veröffentlicht",
        "Mindestens eine unterstützte Metadatenversion als dreiteilige Versionsnummer erwartet.",
        "$.metadataVersions",
        url);
    if (r.apiVersion.equals("v3"))
      r.check(
          Set.of("A", "B", "C").contains(value(d, "type")),
          "Zustellpunkttyp",
          value(d, "type"),
          "$.type",
          url);
    if (r.apiVersion.equals("v2"))
      r.add(
          "notApplicable",
          "Zustellpunkttyp und Prozessnachrichten",
          "Diese Felder werden über v3 ausgewertet.",
          "$",
          url,
          "",
          "");
    String service = value(input, "service"),
        schema = value(input, "schema"),
        mime = value(input, "mime"),
        region = value(input, "region");
    if (!service.isBlank() || !schema.isBlank() || !mime.isBlank() || !region.isBlank()) {
      boolean match = false;
      for (JsonElement item : array(d, "publicServices")) {
        JsonObject s = item.getAsJsonObject();
        boolean id =
            service.isBlank()
                || value(s, "identifier").equals(service)
                || value(s, "identifier").equals("urn:de:fim:leika:leistung:" + service);
        boolean area = region.isBlank() || contains(array(s, "regions"), region);
        if (id && area && schemaMatch(array(s, "submissionSchemas"), schema, mime)) match = true;
      }
      r.check(
          match,
          "Erwartete Verwaltungskonfiguration",
          match
              ? "Leistung, Region und Schema/MIME passen in einem gemeinsamen Leistungskontext."
              : "Kein gemeinsamer Leistungskontext erfüllt alle angegebenen Erwartungen.",
          "$.publicServices",
          url);
    }
    String metadata = value(input, "metadata");
    if (!metadata.isBlank())
      r.check(
          contains(array(d, "metadataVersions"), metadata),
          "Metadatenversion",
          metadata,
          "$.metadataVersions",
          url);
    if (d.has("senderAccessRestricted"))
      r.add(
          "warning",
          "Zugangsregeln prüfen",
          "Veröffentlichte Senderbeschränkungen werden angezeigt. Ohne Identität des Senders ist"
              + " keine Zugangsentscheidung möglich.",
          "$.senderAccessRestricted",
          url,
          "Erlaubte Sender und Zustellpunkte vergleichen.",
          "Keine authentifizierte Sendeprobe.");
  }

  static boolean schemaMatch(JsonArray schemas, String schema, String mime) {
    if (schema.isBlank() && mime.isBlank()) return true;
    for (JsonElement e : schemas) {
      JsonObject s = e.getAsJsonObject();
      if ((schema.isBlank() || schema.equals(value(s, "schemaUri")))
          && (mime.isBlank() || mime.equals(value(s, "mimeType")))) return true;
    }
    return false;
  }

  static boolean contains(JsonArray a, String value) {
    for (JsonElement e : a) if (e.isJsonPrimitive() && e.getAsString().equals(value)) return true;
    return false;
  }

  public static JsonArray array(JsonObject o, String name) {
    return o.has(name) && o.get(name).isJsonArray() ? o.getAsJsonArray(name) : new JsonArray();
  }

  static int counter(JsonObject object, String name) {
    JsonElement value = object.get(name);
    if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber())
      throw new IllegalArgumentException("Ungültiger Seitenzähler.");
    int number = value.getAsBigDecimal().intValueExact();
    if (number < 0) throw new IllegalArgumentException("Negativer Seitenzähler.");
    return number;
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
    JsonArray routes = pages(url, "routes");
    report.data.add("routes", routes);
    Signatures signatures = new Signatures(this);
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
    if (input.has("pvog") && input.get("pvog").getAsBoolean()) pvog(input, signatures);
    if (report.environment.equals("STAGE"))
      report.add(
          "warning",
          "Routingquelle PROD",
          "STAGE hat keinen eigenen Routingdienst. Angezeigt werden PROD-Routen, keine"
              + " STAGE-Zustellbarkeit.",
          "$",
          DOC,
          "STAGE-Destination direkt prüfen.",
          "");
    report.unknown(
        "Öffentliche Routingbeobachtung",
        "Kontexte bleiben erhalten. Abweichungen zu PVOG belegen keinen Fehler einer internen"
            + " Routingentscheidung.",
        "$.routes",
        DOC);
  }

  void pvog(JsonObject input, Signatures signatures) throws IOException {
    String lb = value(input, "lbId"), region = value(input, "region").replaceFirst("^DE", "");
    if (!value(input, "regionKind").equals("ars")
        || !region.matches("[0-9]{12}")
        || lb.isBlank()
        || lb.length() > 255) {
      report.unknown(
          "PVOG-Vergleich benötigt ARS und Leistungsbeschreibungs-ID",
          "LeiKa und Leistungsbeschreibungs-ID sind unterschiedliche Kennungen. Kein Gebiet wird"
              + " geraten.",
          "$",
          "https://pvog.fitko.de/api/suchdienst/");
      return;
    }
    String base =
        report.environment.equals("TEST")
            ? "https://stage.pvog.fitko.net/suchdienst/api"
            : "https://pvog.fitko.net/suchdienst/api";
    String url =
        base
            + "/v1/relations/jzufi-2-3?ars="
            + region
            + "&lbids="
            + encode(lb)
            + "&usearshierarchy=true&size=100";
    JsonArray contexts = new JsonArray();
    boolean complete = false;
    for (int page = 0; page < 10; page++) {
      JsonObject response;
      try {
        response = json(url + "&page=" + page);
      } catch (IOException e) {
        break;
      }
      if (response == null) break;
      JsonArray content = array(response, "content");
      contexts.addAll(content);
      if (!response.has("page")) {
        report.unknown(
            "PVOG-Paginierung unbekannt", "Keine Seitenmetadaten veröffentlicht.", "$", url);
        break;
      }
      JsonObject metadata = response.getAsJsonObject("page");
      int totalPages;
      try {
        totalPages = counter(metadata, "totalPages");
        if (counter(metadata, "number") != page
            || counter(metadata, "totalElements") < contexts.size()
            || !response.has("content")
            || !response.get("content").isJsonArray()) break;
      } catch (RuntimeException invalid) {
        break;
      }
      if (page + 1 >= totalPages) {
        complete = true;
        break;
      }
      if (content.isEmpty()) break;
    }
    report.data.add("pvogRelations", contexts);
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
            : "Teilbestand, kein vollständiger Negativnachweis.");
    int i = 0;
    for (JsonElement context : contexts) {
      int j = 0;
      for (JsonElement relation : array(context.getAsJsonObject(), "zustaendigkeit")) {
        int k = 0;
        for (JsonElement id : array(relation.getAsJsonObject(), "idSekundaer")) {
          JsonObject row = id.getAsJsonObject();
          if (value(row, "schemeID").equals("urn:de:fitko:fit-connect:xzufi:destination"))
            signatures.inspect(
                value(row, "value"),
                "",
                value(input, "service"),
                region,
                "$.pvogRelations[" + i + "].zustaendigkeit[" + j + "].idSekundaer[" + k + "]");
          k++;
        }
        j++;
      }
      i++;
    }
  }

  public void areas(JsonObject input) throws IOException {
    String search = value(input, "areaSearch");
    if (search.length() < 2 || search.length() > 150)
      throw new IllegalArgumentException("Gebietssuche mit 2 bis 150 Zeichen erwartet.");
    report.data.add(
        "areas",
        pages(
            routing(report.environment) + "/v2/areas?areaSearchexpression=" + encode(search),
            "areas"));
  }

  public void process(JsonObject input) throws IOException {
    if (!report.apiVersion.equals("v3"))
      throw new IllegalArgumentException("Prozesssuche benötigt v3.");
    String model = value(input, "processModelId"),
        message = value(input, "messageId"),
        region = value(input, "region");
    if (model.isBlank()
        || model.length() > 512
        || message.isBlank()
        || message.length() > 255
        || !region.matches("DE[0-9]{0,12}"))
      throw new IllegalArgumentException(
          "Prozessmodell, Nachricht und Region DE mit bis zu 12 Ziffern erwartet.");
    String url =
        submission(report.environment)
            + "/v3/search-destinations/by-process-message?processModelId="
            + encode(model)
            + "&messageId="
            + encode(message)
            + "&region="
            + region;
    JsonArray destinations = pages(url, "destinations");
    report.data.add("destinations", destinations);
    int index = 0;
    for (JsonElement item : destinations) {
      JsonObject destination = item.getAsJsonObject();
      boolean matched = false;
      for (JsonElement element : array(destination, "processMessages")) {
        JsonObject context = element.getAsJsonObject();
        if (value(context, "processModelId").equals(model)
            && value(context, "messageId").equals(message)
            && regionMatches(array(context, "regions"), region)
            && schemaMatch(array(context, "schemas"), value(input, "schema"), value(input, "mime")))
          matched = true;
      }
      report.check(
          matched,
          "Prozesskontext passt",
          value(destination, "destinationId")
              + ": Modell, Nachricht, Region und optionale Schemainformation müssen im selben"
              + " Nachrichtenkontext passen.",
          "$.destinations[" + (index++) + "].processMessages",
          url);
    }
    String from = value(input, "fromDestinationId"), to = value(input, "toDestinationId");
    if (!from.isBlank() || !to.isBlank()) {
      JsonObject limits =
          json(submission(report.environment) + "/v3/limits/" + uuid(from) + "/" + uuid(to));
      if (limits != null) report.data.add("limits", limits);
    }
  }

  static boolean regionMatches(JsonArray published, String requested) {
    for (JsonElement region : published)
      if (region.isJsonPrimitive()
          && region.getAsString().matches("DE[0-9]{0,12}")
          && requested.startsWith(region.getAsString())) return true;
    return false;
  }
}
