package de.fbforge.inspect;

import static de.fbforge.inspect.JsonFields.*;

import com.google.gson.*;
import java.io.IOException;
import java.time.Instant;

/** Public-side readiness of an explicitly named replacement key. Never accesses private keys. */
final class RolloverDiagnostics {
  static void inspect(Api api, PublicHttp http, JsonObject input) throws IOException {
    Report report = api.report;
    String id = uuid(value(input, "destinationId")), candidate = value(input, "candidateKid");
    if (candidate.isBlank()
        || candidate.length() > 255
        || candidate.codePoints().anyMatch(Character::isISOControl))
      throw new IllegalArgumentException(
          "Neue öffentliche Schlüsselkennung mit höchstens 255 Zeichen angeben.");
    String base =
        Api.submission(report.environment) + "/" + report.apiVersion + "/destinations/" + id;
    JsonObject destination = api.json(base);
    if (destination == null) return;
    report.data.add("destination", destination);
    String active = value(destination, "encryptionKid");
    JsonObject overview = new JsonObject();
    overview.addProperty("activeKid", active);
    overview.addProperty("candidateKid", candidate);
    report.data.add("rollover", overview);
    if (active.isBlank()) {
      report.unknown(
          "Aktiver Verschlüsselungsschlüssel fehlt",
          "Es ist keine aktive Schlüsselkennung veröffentlicht.",
          "$.encryptionKid",
          base);
      return;
    }
    JsonArray keys = new JsonArray();
    String activeThumbprint = "", candidateThumbprint = "";
    for (String kid : new String[] {active, candidate}) {
      String source = base + "/keys/" + pathSegment(kid);
      JsonObject key = api.json(source);
      if (key == null) continue;
      try {
        String fingerprint =
            com.nimbusds.jose.jwk.JWK.parse(key.toString()).computeThumbprint().toString();
        if (kid.equals(active)) activeThumbprint = fingerprint;
        if (kid.equals(candidate)) candidateThumbprint = fingerprint;
      } catch (Exception invalid) {
        // The regular key inspection retains the invalid-key evidence without input values.
      }
      report.check(
          kid.equals(value(key, "kid")),
          "Abgerufene Schlüsselkennung",
          "Die Antwort muss zur angefragten Schlüsselkennung passen.",
          "$.kid",
          source);
      report.check(
          value(key, "alg").equals("RSA-OAEP-256"),
          "Verschlüsselung für Schlüsselwechsel",
          "Der ausgewählte Schlüssel muss RSA-OAEP-256 verwenden.",
          "$.alg",
          source);
      KeyDiagnostics.inspect(key.toString(), report, http, input);
      keys.addAll(array(report.data, "keys"));
      if (active.equals(candidate)) break;
    }
    report.data.add("keys", keys);
    if (!activeThumbprint.isBlank() && !candidateThumbprint.isBlank())
      report.check(
          !activeThumbprint.equals(candidateThumbprint),
          "Neuer Schlüsselinhalt",
          activeThumbprint.equals(candidateThumbprint)
              ? "Die ausgewerteten öffentlichen Schlüssel haben denselben Inhalt. Eine andere"
                    + " Kennung allein ist kein Schlüsselwechsel."
              : "Die ausgewerteten öffentlichen Schlüssel haben unterschiedlichen Inhalt.",
          "$.rollover",
          base);
    else
      report.unknown(
          "Neuer Schlüsselinhalt nicht nachgewiesen",
          "Beide öffentlichen Schlüssel müssen auswertbar sein.",
          "$.rollover",
          base);
    report.check(
        !active.equals(candidate),
        "Neuer Schlüssel ausgewählt",
        active.equals(candidate)
            ? "Die neue Kennung entspricht bereits dem aktiven Schlüssel."
            : "Aktiver und ausgewählter neuer Schlüssel haben unterschiedliche Kennungen.",
        "$.rollover",
        base);
    Instant oldEnd = null, newStart = null, newEnd = null;
    for (JsonElement element : array(report.data, "certificates")) {
      JsonObject cert = element.getAsJsonObject();
      if (cert.get("chainIndex").getAsInt() != 0) continue;
      if (value(cert, "kid").equals(active)) oldEnd = Instant.parse(value(cert, "notAfter"));
      if (value(cert, "kid").equals(candidate)) {
        newStart = Instant.parse(value(cert, "notBefore"));
        newEnd = Instant.parse(value(cert, "notAfter"));
      }
    }
    if (oldEnd != null && newStart != null && newEnd != null) {
      boolean overlap =
          oldEnd.isAfter(newStart)
              && newEnd.isAfter(Instant.now())
              && oldEnd.isAfter(Instant.now());
      overview.addProperty("validityOverlap", overlap);
      report.check(
          overlap,
          "Gültigkeitsüberlappung",
          overlap
              ? "Die ausgewerteten Teilnehmerzertifikate besitzen einen nutzbaren überlappenden"
                    + " Zeitraum."
              : "Ein nutzbarer überlappender Zeitraum ist nicht bestätigt.",
          "$.rollover",
          base);
    } else
      report.unknown(
          "Gültigkeitsüberlappung nicht nachgewiesen",
          "Für beide Schlüssel werden auswertbare Teilnehmerzertifikate benötigt.",
          "$.rollover",
          base);
    report.add(
        "warning",
        "Empfangssystem separat prüfen",
        "Private Schlüsselbestände und erfolgreiche Entschlüsselung sind aus der öffentlichen"
            + " Konfiguration nicht nachweisbar.",
        "$.rollover",
        base,
        "Den Schlüsselwechsel im Empfangssystem vorbereiten und bisherige private Schlüssel nach"
            + " Betriebsvorgabe aufbewahren.",
        "Keine Änderung am Zustellpunkt und keine Zustellprobe ausgeführt.");
  }
}
