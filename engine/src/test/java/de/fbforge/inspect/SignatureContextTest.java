package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.nimbusds.jose.*;
import com.nimbusds.jose.jwk.*;
import com.nimbusds.jwt.*;
import java.io.IOException;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.*;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class SignatureContextTest {
  private static Report report(String environment) {
    return new Report("routing", environment, "v3");
  }

  private static void finding(Report report, String title, String status) {
    assertTrue(
        report.findings.stream()
            .anyMatch(f -> f.title().equals(title) && f.status().equals(status)),
        title + " / " + report.findings);
  }

  private static void inspect(
      Report report, JWTClaimsSet claims, String destination, String service, String region)
      throws Exception {
    Api api = new Api(report, url -> SignatureFixtures.response(url, SignatureFixtures.jwks()));
    new Signatures(api)
        .inspect(SignatureFixtures.token(claims), destination, service, region, "$.routes[0]");
  }

  @ParameterizedTest
  @ValueSource(strings = {"TEST", "STAGE", "PROD"})
  void verifiesBoundContextsAgainstTheEnvironmentSource(String environment) throws Exception {
    Report report = report(environment);
    List<String> sources = new ArrayList<>();
    Api api =
        new Api(
            report,
            url -> {
              sources.add(url);
              return SignatureFixtures.response(url, SignatureFixtures.jwks());
            });
    new Signatures(api)
        .inspect(
            SignatureFixtures.token(SignatureFixtures.claims(environment).build()),
            SignatureFixtures.DESTINATION,
            SignatureFixtures.SERVICE,
            SignatureFixtures.ARS,
            "$.routes[0]");
    assertEquals(
        List.of(
            "https://"
                + (environment.equals("TEST")
                    ? "test.fit-connect.fitko.dev"
                    : "prod.fit-connect.fitko.net")
                + "/.well-known/jwks.json"),
        sources);
    for (String title :
        List.of(
            "Adressierungssignatur",
            "Signierter Zustellpunkt",
            "Signierter Zustelldienst",
            "Zeitangaben der Adressierung",
            "Signierter Leistungskontext")) finding(report, title, "confirmed");
  }

  @Test
  void reportsHostDestinationTimeAndServiceMismatchesSeparately() throws Exception {
    Report report = report("TEST");
    inspect(
        report,
        SignatureFixtures.claims("TEST")
            .claim("submissionHost", "foreign.example")
            .expirationTime(Date.from(Instant.now().minusSeconds(3600)))
            .build(),
        "foreign-destination",
        "00000000000000",
        SignatureFixtures.ARS);
    finding(report, "Adressierungssignatur", "confirmed");
    for (String title :
        List.of(
            "Signierter Zustellpunkt",
            "Signierter Zustelldienst",
            "Zeitangaben der Adressierung",
            "Signierter Leistungskontext")) finding(report, title, "error");
  }

  @Test
  void doesNotCombineServicesAndRegionsAcrossDifferentSignedContexts() throws Exception {
    var claims =
        SignatureFixtures.claims("TEST")
            .claim(
                "services",
                List.of(
                    Map.of(
                        "leistungIDs",
                        List.of("urn:de:fim:leika:leistung:" + SignatureFixtures.SERVICE),
                        "gebietIDs",
                        List.of("urn:de:bund:destatis:bevoelkerungsstatistik:schluessel:rs:09")),
                    Map.of(
                        "leistungIDs",
                        List.of("urn:de:fim:leika:leistung:00000000000000"),
                        "gebietIDs",
                        List.of("urn:de:bund:destatis:bevoelkerungsstatistik:schluessel:rs:02"))))
            .build();
    Report report = report("TEST");
    inspect(report, claims, "", SignatureFixtures.SERVICE, SignatureFixtures.ARS);
    finding(report, "Signierter Leistungskontext", "error");
  }

  @Test
  void missingIssueTimeFutureIssueTimeAndMissingJtiRemainErrors() throws Exception {
    for (var claims :
        List.of(
            SignatureFixtures.claims("TEST").issueTime(null).build(),
            SignatureFixtures.claims("TEST")
                .issueTime(Date.from(Instant.now().plusSeconds(3600)))
                .build(),
            SignatureFixtures.claims("TEST").jwtID(null).build())) {
      Report report = report("TEST");
      inspect(report, claims, "", SignatureFixtures.SERVICE, SignatureFixtures.ARS);
      finding(report, "Zeitangaben der Adressierung", "error");
    }
  }

  @Test
  void absentExpiryIsAllowedAndArsIsNotGuessedFromAgs() throws Exception {
    Report report = report("TEST");
    inspect(
        report,
        SignatureFixtures.claims("TEST").expirationTime(null).build(),
        "",
        SignatureFixtures.SERVICE,
        "");
    finding(report, "Zeitangaben der Adressierung", "confirmed");
    finding(report, "Signierte Zuständigkeit nicht verglichen", "unknown");
  }

  @Test
  void unknownOrDuplicateKidsNeverConfirmAKey() throws Exception {
    for (String keys :
        List.of(
            "{\"keys\":[]}",
            new JWKSet(
                    List.of(
                        SignatureFixtures.key().toPublicJWK(),
                        SignatureFixtures.key().toPublicJWK()))
                .toString())) {
      Report report = report("TEST");
      Signatures signatures =
          new Signatures(new Api(report, url -> SignatureFixtures.response(url, keys)));
      signatures.inspect(
          SignatureFixtures.token(SignatureFixtures.claims("TEST").build()), "", "", "", "$");
      finding(report, "Adressierungsschlüssel nicht eindeutig verfügbar", "unknown");
      assertFalse(
          report.findings.stream()
              .anyMatch(
                  f ->
                      f.title().equals("Adressierungssignatur") && f.status().equals("confirmed")));
    }
  }

  @Test
  void invalidAlgorithmsIssuersAndMissingKidDoNotVerify() throws Exception {
    for (String token :
        List.of(
            SignatureFixtures.token(
                SignatureFixtures.claims("TEST").build(), JWSAlgorithm.RS256, "fixture"),
            SignatureFixtures.token(
                SignatureFixtures.claims("TEST").issuer("https://foreign.example").build()),
            SignatureFixtures.token(
                SignatureFixtures.claims("TEST").build(), JWSAlgorithm.PS512, null))) {
      Report report = report("TEST");
      new Signatures(
              new Api(report, url -> SignatureFixtures.response(url, SignatureFixtures.jwks())))
          .inspect(token, "", "", "", "$");
      assertFalse(
          report
              .data
              .getAsJsonArray("addressingSignatures")
              .get(0)
              .getAsJsonObject()
              .get("verified")
              .getAsBoolean());
    }
  }

  @Test
  void fetchFailuresAreCachedAndSignatureBudgetIsBounded() throws Exception {
    Report report = report("TEST");
    AtomicInteger fetches = new AtomicInteger();
    Signatures signatures =
        new Signatures(
            new Api(
                report,
                url -> {
                  fetches.incrementAndGet();
                  throw new IOException("fixture");
                }));
    String token = SignatureFixtures.token(SignatureFixtures.claims("TEST").build());
    for (int i = 0; i < 22; i++) signatures.inspect(token, "", "", "", "$.routes[" + i + "]");
    assertEquals(1, fetches.get());
    assertEquals(20, report.data.getAsJsonArray("addressingSignatures").size());
    finding(report, "Signaturbudget erreicht", "unknown");
  }

  @ParameterizedTest
  @ValueSource(strings = {"invalid", "a.b.c"})
  void malformedTokensRemainUnknown(String token) {
    Report report = report("TEST");
    new Signatures(
            new Api(
                report,
                url -> {
                  fail("no fetch for malformed JWT");
                  return null;
                }))
        .inspect(token, "", "", "", "$");
    finding(report, "Adressierungssignatur nicht auswertbar", "unknown");
  }
}
