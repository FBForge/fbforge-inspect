package de.fbforge.inspect;

import com.google.gson.*;
import com.nimbusds.jose.*;
import com.nimbusds.jose.crypto.RSASSAVerifier;
import com.nimbusds.jose.jwk.*;
import com.nimbusds.jwt.*;
import java.net.URI;
import java.time.Instant;
import java.util.*;

/** Decoded JWT claims remain untrusted until the independently selected source verifies them. */
final class Signatures {
  private final Api api;
  private JsonObject jwks;
  private boolean attempted;
  private int count;

  Signatures(Api api) {
    this.api = api;
  }

  void inspect(
      String compact, String expectedDestination, String service, String region, String path) {
    Report r = api.report;
    if (++count > 20) {
      if (count == 21)
        r.unknown(
            "Signaturbudget erreicht",
            "Maximal 20 Adressierungssignaturen pro Lauf geprüft. Weitere Kontexte bleiben"
                + " ungeprüft.",
            path,
            Api.DOC);
      return;
    }
    String env = r.environment.equals("TEST") ? "TEST" : "PROD";
    String host = URI.create(Api.submission(env)).getHost(),
        source = "https://" + host + "/.well-known/jwks.json";
    try {
      if (compact.length() > 65536) throw new IllegalArgumentException();
      SignedJWT jwt = SignedJWT.parse(compact);
      JWTClaimsSet claims = jwt.getJWTClaimsSet();
      JsonObject decoded = new JsonObject();
      decoded.addProperty("context", path);
      decoded.addProperty("verified", false);
      decoded.add("claims", JsonParser.parseString(claims.toString()));
      if (!r.data.has("addressingSignatures")) r.data.add("addressingSignatures", new JsonArray());
      r.data.getAsJsonArray("addressingSignatures").add(decoded);
      if (!attempted) {
        attempted = true;
        jwks = api.json(source);
      }
      if (jwks == null) {
        r.unknown(
            "Adressierungssignatur nicht verifiziert",
            "Claims wurden nur dekodiert. Vertrauenswürdiger öffentlicher Schlüsselabruf fehlt.",
            path,
            source);
        return;
      }
      JWKSet set = JWKSet.parse(jwks.toString());
      String kid = jwt.getHeader().getKeyID();
      List<JWK> matches =
          set.getKeys().stream().filter(k -> kid != null && kid.equals(k.getKeyID())).toList();
      boolean verified =
          matches.size() == 1 && matches.getFirst() instanceof RSAKey key && verify(jwt, key, env);
      decoded.addProperty("verified", verified);
      r.check(
          verified,
          "Adressierungssignatur",
          verified
              ? "PS512 mit unabhängig gewähltem FIT-Connect-Schlüssel verifiziert."
              : "Algorithmus, Schlüssel, Aussteller oder Signatur ungültig.",
          path,
          source);
      if (!verified) return;
      r.check(
          expectedDestination.isBlank()
              || expectedDestination.equals(claims.getStringClaim("destinationId")),
          "Signierter Zustellpunkt",
          "destinationId muss dem Kontext entsprechen.",
          path + ".destinationId",
          source);
      r.check(
          host.equals(claims.getStringClaim("submissionHost")),
          "Signierter Zustelldienst",
          "submissionHost muss zur gewählten Routingumgebung passen.",
          path + ".submissionHost",
          source);
      Date iat = claims.getIssueTime(), exp = claims.getExpirationTime();
      Instant now = Instant.now();
      r.check(
          iat != null
              && !iat.toInstant().isAfter(now.plusSeconds(300))
              && (exp == null || !exp.toInstant().isBefore(now.minusSeconds(300)))
              && claims.getJWTID() != null
              && !claims.getJWTID().isBlank(),
          "Zeitangaben der Adressierung",
          "iat darf nicht in der Zukunft liegen. exp wird berücksichtigt, falls vorhanden. jti muss"
              + " vorliegen.",
          path,
          source);
      if (!service.isBlank() && !region.isBlank()) {
        JsonObject payload = JsonParser.parseString(claims.toString()).getAsJsonObject();
        boolean matched = false;
        for (JsonElement e : Api.array(payload, "services")) {
          JsonObject context = e.getAsJsonObject();
          boolean
              serviceMatch =
                  Api.contains(
                      Api.array(context, "leistungIDs"), "urn:de:fim:leika:leistung:" + service),
              areaMatch = false;
          for (JsonElement area : Api.array(context, "gebietIDs")) {
            String prefix = "urn:de:bund:destatis:bevoelkerungsstatistik:schluessel:rs:",
                value = area.getAsString();
            if (value.startsWith(prefix)) {
              String ars = value.substring(prefix.length());
              if (ars.matches("[0-9]{0,12}") && region.startsWith(ars)) areaMatch = true;
            }
          }
          if (serviceMatch && areaMatch) matched = true;
        }
        r.check(
            matched,
            "Signierter Leistungskontext",
            "Leistung und ARS müssen im selben signierten Kontext passen.",
            path + ".services",
            source);
      } else
        r.unknown(
            "Signierte Zuständigkeit nicht verglichen",
            "Für AGS oder Gebiets-ID wird keine ARS-Umrechnung geraten.",
            path + ".services",
            source);
    } catch (Exception e) {
      r.unknown(
          "Adressierungssignatur nicht auswertbar",
          "JWT-Format, Claims oder veröffentlichte Schlüssel konnten nicht sicher ausgewertet"
              + " werden.",
          path,
          source);
    }
  }

  static boolean verify(SignedJWT jwt, RSAKey key, String env) throws Exception {
    String issuer = jwt.getJWTClaimsSet().getIssuer();
    String root = URI.create(Api.submission(env)).getHost();
    String portal =
        env.equals("TEST")
            ? "portal.auth-testing.fit-connect.fitko.dev"
            : "portal.auth-prod.fit-connect.fitko.net";
    boolean knownIssuer =
        Set.of(
                "https://" + portal,
                "https://" + root,
                "https://" + portal + "/",
                "https://" + root + "/")
            .contains(issuer == null ? "" : issuer);
    return knownIssuer
        && JWSAlgorithm.PS512.equals(jwt.getHeader().getAlgorithm())
        && key.size() == 4096
        && !key.isPrivate()
        && (jwt.getHeader().getCriticalParams() == null
            || jwt.getHeader().getCriticalParams().isEmpty())
        && jwt.verify(new RSASSAVerifier(key.toRSAPublicKey()));
  }
}
