package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import com.nimbusds.jose.JWEAlgorithm;
import com.nimbusds.jose.jwk.*;
import java.net.*;
import java.util.*;
import org.junit.jupiter.api.*;

class RolloverDiagnosticsTest {
  static final String ID = SignatureFixtures.DESTINATION;

  @BeforeAll
  static void fixtures() throws Exception {
    CryptoTest.fixtures();
  }

  static String key(String kid) throws Exception {
    return new RSAKey.Builder(
            (java.security.interfaces.RSAPublicKey) CryptoTest.leafKey.getPublic())
        .keyID(kid)
        .algorithm(JWEAlgorithm.RSA_OAEP_256)
        .keyOperations(Set.of(KeyOperation.WRAP_KEY))
        .x509CertChain(
            List.of(
                com.nimbusds.jose.util.Base64.encode(CryptoTest.leaf.getEncoded()),
                com.nimbusds.jose.util.Base64.encode(CryptoTest.issuer.getEncoded())))
        .build()
        .toJSONString();
  }

  static JsonObject input(String candidate) {
    JsonObject input = new JsonObject();
    input.addProperty("destinationId", ID);
    input.addProperty("candidateKid", candidate);
    input.addProperty("revocation", false);
    return input;
  }

  static PublicHttp http(Report report, String candidate, boolean missing) throws Exception {
    String activeKey = key("active"), nextKey = key(candidate);
    return new PublicHttp(
        report,
        host -> new InetAddress[] {InetAddress.getByAddress(new byte[] {8, 8, 8, 8})},
        (uri, ip, post, budget) -> {
          String body =
              uri.getPath().endsWith("/keys/active")
                  ? activeKey
                  : uri.getPath().contains("/keys/")
                      ? nextKey
                      : "{\"destinationId\":\"" + ID + "\",\"encryptionKid\":\"active\"}";
          if (missing && uri.getPath().endsWith("/keys/next"))
            return new PublicHttp.Response(404, Map.of(), new byte[0], uri);
          return SignatureFixtures.response(uri.toString(), body);
        },
        System::nanoTime);
  }

  @Test
  void checksBothExplicitKeysWithoutMutatingTheDestination() throws Exception {
    Report report = new Report("rollover", "TEST", "v3");
    PublicHttp http = http(report, "next", false);
    RolloverDiagnostics.inspect(new Api(report, http::get), http, input("next"));
    assertEquals(2, report.data.getAsJsonArray("keys").size());
    assertTrue(report.data.getAsJsonObject("rollover").get("validityOverlap").getAsBoolean());
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f -> f.title().equals("Neuer Schlüsselinhalt") && f.status().equals("error")));
    assertTrue(report.requests.stream().allMatch(r -> r.method().equals("GET")));
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("Empfangssystem separat prüfen")
                        && f.status().equals("warning")));
    assertEquals(
        "active", report.data.getAsJsonObject("destination").get("encryptionKid").getAsString());
  }

  @Test
  void aMissingReplacementKeyDoesNotBecomeAReadinessConfirmation() throws Exception {
    Report report = new Report("rollover", "PROD", "v3");
    PublicHttp http = http(report, "next", true);
    RolloverDiagnostics.inspect(new Api(report, http::get), http, input("next"));
    assertFalse(report.data.getAsJsonObject("rollover").has("validityOverlap"));
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f -> f.status().equals("unknown") && f.title().contains("Gültigkeitsüberlappung")));
  }

  @Test
  void selectingTheCurrentKeyCannotClaimANewKey() throws Exception {
    Report report = new Report("rollover", "TEST", "v3");
    PublicHttp http = http(report, "active", false);
    RolloverDiagnostics.inspect(new Api(report, http::get), http, input("active"));
    assertEquals(1, report.data.getAsJsonArray("keys").size());
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f -> f.title().equals("Neuer Schlüssel ausgewählt") && f.status().equals("error")));
  }

  @Test
  void malformedCandidateAndCredentialTokensAreRejectedBeforeFetching() throws Exception {
    Report report = new Report("rollover", "TEST", "v3");
    PublicHttp http = http(report, "next", false);
    assertThrows(
        IllegalArgumentException.class,
        () -> RolloverDiagnostics.inspect(new Api(report, http::get), http, input("")));
    assertTrue(report.requests.isEmpty());
    assertThrows(
        IllegalArgumentException.class,
        () -> PublicSignatureInput.validate("synthetic-access-token"));
    PublicSignatureInput.validate(
        SignatureFixtures.token(SignatureFixtures.claims("TEST").build()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            PublicSignatureInput.validate(
                SignatureFixtures.token(
                    new com.nimbusds.jwt.JWTClaimsSet.Builder()
                        .subject("synthetic-user")
                        .build())));
  }
}
