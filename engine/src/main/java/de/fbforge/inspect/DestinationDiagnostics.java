package de.fbforge.inspect;

import static de.fbforge.inspect.Api.submission;
import static de.fbforge.inspect.JsonFields.*;

import com.google.gson.*;
import java.io.IOException;
import java.util.*;

/** Destination configuration and comparison rules, independent of presentation. */
final class DestinationDiagnostics {
  private final Api api;
  private final Report report;

  DestinationDiagnostics(Api api) {
    this.api = api;
    this.report = api.report;
  }

  public JsonObject destination(JsonObject input, PublicHttp http) throws IOException {
    String id = uuid(value(input, "destinationId")),
        url = submission(report.environment) + "/" + report.apiVersion + "/destinations/" + id;
    JsonObject d = api.json(url);
    if (d == null) return null;
    report.data.add("destination", d);
    inspectDestination(d, input, report, url);
    String kid = value(d, "encryptionKid");
    if (!kid.isBlank()) {
      String keyUrl = url + "/keys/" + encode(kid);
      JsonObject key = api.json(keyUrl);
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
      JsonObject c = api.json(submission(report.environment) + "/" + other + "/destinations/" + id);
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
          FindingCode.DESTINATION_ID,
          value(input, "destinationId").equalsIgnoreCase(value(d, "destinationId")),
          "Destination-ID",
          "Antwort-ID: "
              + value(d, "destinationId")
              + ". Angefragt: "
              + value(input, "destinationId")
              + ".",
          "$.destinationId",
          url);
    r.check(
        FindingCode.DESTINATION_STATUS,
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
        "Veröffentlicht: "
            + versions
            + ". Erwartet wird mindestens eine dreiteilige Versionsnummer.",
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
}
