package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import java.io.IOException;
import java.math.BigInteger;
import java.net.URI;
import java.security.cert.X509Certificate;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.atomic.AtomicInteger;
import org.bouncycastle.asn1.x500.X500Name;
import org.bouncycastle.asn1.x509.*;
import org.bouncycastle.cert.jcajce.*;
import org.bouncycastle.cert.ocsp.*;
import org.bouncycastle.operator.jcajce.JcaContentSignerBuilder;
import org.junit.jupiter.api.*;

class RevocationWorkflowTest {
  @BeforeAll
  static void fixtures() throws Exception {
    CryptoTest.fixtures();
  }

  static X509Certificate leaf(boolean ocsp, String... crls) throws Exception {
    var builder =
        new JcaX509v3CertificateBuilder(
            new X500Name("CN=Synthetic Test CA"),
            BigInteger.TWO,
            Date.from(Instant.now().minusSeconds(3600)),
            Date.from(Instant.now().plusSeconds(3600)),
            new X500Name("CN=Synthetic Endpoint Leaf"),
            CryptoTest.leafKey.getPublic());
    if (ocsp)
      builder.addExtension(
          org.bouncycastle.asn1.x509.Extension.authorityInfoAccess,
          false,
          new AuthorityInformationAccess(
              AccessDescription.id_ad_ocsp,
              new GeneralName(GeneralName.uniformResourceIdentifier, "https://example.org/ocsp")));
    if (crls.length > 0)
      builder.addExtension(
          org.bouncycastle.asn1.x509.Extension.cRLDistributionPoints,
          false,
          new CRLDistPoint(
              Arrays.stream(crls)
                  .map(
                      url ->
                          new DistributionPoint(
                              new DistributionPointName(
                                  new GeneralNames(
                                      new GeneralName(GeneralName.uniformResourceIdentifier, url))),
                              null,
                              null))
                  .toArray(DistributionPoint[]::new)));
    return new JcaX509CertificateConverter()
        .getCertificate(
            builder.build(
                new JcaContentSignerBuilder("SHA256withRSA")
                    .build(CryptoTest.issuerKey.getPrivate())));
  }

  private static PublicHttp.Response response(String url, byte[] bytes) {
    return new PublicHttp.Response(200, Map.of(), bytes, URI.create(url));
  }

  private static byte[] crl(boolean revoked) throws Exception {
    return CryptoTest.crl(
        revoked,
        CryptoTest.issuerKey,
        CryptoTest.now.minusSeconds(60),
        CryptoTest.now.plusSeconds(600));
  }

  @Test
  void acquiresAndVerifiesBothPositiveSourcesWithTheCorrectOcspRequest() throws Exception {
    Report report = new Report("keys", "TEST", "v3");
    byte[]
        ocsp =
            CryptoTest.ocsp(
                null,
                CryptoTest.issuerKey,
                CryptoTest.id,
                CryptoTest.now.minusSeconds(60),
                CryptoTest.now.plusSeconds(600)),
        crl = crl(false);
    AtomicInteger calls = new AtomicInteger();
    PkiFetch fetch =
        (url, post) -> {
          calls.incrementAndGet();
          if (url.endsWith("ocsp")) {
            assertNotNull(post);
            assertEquals(CryptoTest.id, new OCSPReq(post).getRequestList()[0].getCertID());
            return response(url, ocsp);
          }
          assertNull(post);
          return response(url, crl);
        };
    CertificateRevocation.revocation(
        leaf(true, "https://example.org/crl"), CryptoTest.issuer, report, fetch, "$.keys[0]");
    assertEquals(2, calls.get());
    assertEquals(2, report.findings.stream().filter(f -> f.status().equals("confirmed")).count());
  }

  @Test
  void ldapLocationsDoNotConsumeTheHttpCrlBudget() throws Exception {
    var points =
        new CRLDistPoint(
            new DistributionPoint[] {
              new DistributionPoint(
                  new DistributionPointName(
                      new GeneralNames(
                          new GeneralName[] {
                            new GeneralName(
                                GeneralName.uniformResourceIdentifier,
                                "ldap://directory.example.org/one"),
                            new GeneralName(
                                GeneralName.uniformResourceIdentifier,
                                "ldap://directory.example.org/two"),
                            new GeneralName(
                                GeneralName.uniformResourceIdentifier,
                                "http://example.org/current.crl")
                          })),
                  null,
                  null)
            });
    assertEquals(List.of("http://example.org/current.crl"), CrlEndpoints.locations(points));
    Report report = new Report("keys", "PROD", "v3");
    List<String> requested = new ArrayList<>();
    byte[] body = crl(false);
    CertificateRevocation.revocation(
        leaf(
            false,
            "ldap://directory.example.org/one",
            "ldap://directory.example.org/two",
            "http://example.org/current.crl"),
        CryptoTest.issuer,
        report,
        (url, post) -> {
          requested.add(url);
          return response(url, body);
        },
        "$.keys[0]");
    assertEquals(List.of("http://example.org/current.crl"), requested);
    assertTrue(
        report.findings.stream()
            .anyMatch(f -> f.title().equals("CRL-Sperrstatus") && f.status().equals("confirmed")));
  }

  @Test
  void signedRevokedResponsesProduceErrors() throws Exception {
    byte[]
        ocsp =
            CryptoTest.ocsp(
                new RevokedStatus(Date.from(CryptoTest.now.minusSeconds(300)), 1),
                CryptoTest.issuerKey,
                CryptoTest.id,
                CryptoTest.now.minusSeconds(60),
                CryptoTest.now.plusSeconds(600)),
        crl = crl(true);
    Report report = new Report("keys", "TEST", "v3");
    CertificateRevocation.revocation(
        leaf(true, "https://example.org/crl"),
        CryptoTest.issuer,
        report,
        (url, post) -> response(url, post == null ? crl : ocsp),
        "$");
    assertEquals(2, report.findings.stream().filter(f -> f.status().equals("error")).count());
  }

  @Test
  void networkFailuresAndMalformedResponsesCannotProduceGoodStatus() throws Exception {
    Report report = new Report("keys", "TEST", "v3");
    CertificateRevocation.revocation(
        leaf(true, "https://example.org/crl"),
        CryptoTest.issuer,
        report,
        (url, post) -> {
          if (post == null) throw new IOException("fixture");
          return new PublicHttp.Response(503, Map.of(), new byte[0], URI.create(url));
        },
        "$");
    assertEquals(2, report.findings.stream().filter(f -> f.status().equals("unknown")).count());
    assertFalse(report.findings.stream().anyMatch(f -> f.status().equals("confirmed")));
    CertificateRevocation.revocation(
        leaf(true),
        CryptoTest.issuer,
        report,
        (url, post) -> response(url, new byte[] {1, 2, 3}),
        "$");
    assertTrue(
        report.findings.stream().anyMatch(f -> f.title().equals("OCSP nicht verifizierbar")));
  }

  @Test
  void reportsHttpFailuresWithoutMisclassifyingThemAsInvalidCrls() throws Exception {
    Report report = new Report("keys", "PROD", "v3");
    byte[] ocsp =
        CryptoTest.ocsp(
            null,
            CryptoTest.issuerKey,
            CryptoTest.id,
            CryptoTest.now.minusSeconds(60),
            CryptoTest.now.plusSeconds(600));
    CertificateRevocation.revocation(
        leaf(true, "https://example.org/crl"),
        CryptoTest.issuer,
        report,
        (url, post) ->
            post != null
                ? response(url, ocsp)
                : new PublicHttp.Response(
                    500, Map.of(), "do-not-print-server-body".getBytes(), URI.create(url)),
        "$.keys[0].x5c[0]");
    assertTrue(
        report.findings.stream()
            .anyMatch(f -> f.title().equals("OCSP-Sperrstatus") && f.status().equals("confirmed")));
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("CRL-Abruf nicht erfolgreich")
                        && f.status().equals("unknown")
                        && f.detail().contains("HTTP 500")
                        && f.path().equals("$.keys[0].x5c[0]")));
    assertFalse(
        report.findings.stream().anyMatch(f -> f.title().equals("CRL nicht verifizierbar")));
    assertFalse(Main.JSON.toJson(report).contains("do-not-print"));
  }

  @Test
  void safeNetworkFailureExplainsTheFailedFetchWithoutProviderMessages() throws Exception {
    Report report = new Report("keys", "PROD", "v3");
    CertificateRevocation.revocation(
        leaf(false, "https://example.org/crl"),
        CryptoTest.issuer,
        report,
        (url, post) -> {
          throw new NetworkFailure(
              "PROXY_AUTH_FAILED", new IOException("do-not-print-provider-message"));
        },
        "$");
    assertEquals("CRL-Abruf fehlgeschlagen", report.findings.getFirst().title());
    assertTrue(report.findings.getFirst().detail().contains("Proxy-Anmeldung"));
    assertFalse(Main.JSON.toJson(report).contains("do-not-print"));
    assertTrue(report.findings.stream().allMatch(f -> f.status().equals("unknown")));
  }

  @Test
  void malformedSuccessfulCrlResponseIsDistinguishedFromAnHttpFailure() throws Exception {
    Report report = new Report("keys", "PROD", "v3");
    CertificateRevocation.revocation(
        leaf(false, "https://example.org/crl"),
        CryptoTest.issuer,
        report,
        (url, post) -> response(url, new byte[] {1, 2, 3}),
        "$");
    assertEquals("CRL nicht verifizierbar", report.findings.getFirst().title());
    assertTrue(report.findings.getFirst().detail().contains("erhalten"));
    assertTrue(report.findings.getFirst().detail().contains("HTTP 200"));
    assertEquals("unknown", report.findings.getFirst().status());
  }

  @Test
  void absentEndpointsStayUnknownAndCrlLocationsAreBounded() throws Exception {
    Report report = new Report("keys", "PROD", "v3");
    CertificateRevocation.revocation(
        leaf(false),
        CryptoTest.issuer,
        report,
        (url, post) -> {
          fail("no implicit fetch without endpoint");
          return null;
        },
        "$");
    assertTrue(
        report.findings.stream()
            .anyMatch(f -> f.title().equals("Kein unterstützter Sperrendpunkt")));
    AtomicInteger calls = new AtomicInteger();
    CertificateRevocation.revocation(
        leaf(false, "https://example.org/1", "https://example.org/2", "https://example.org/3"),
        CryptoTest.issuer,
        report,
        (url, post) -> {
          calls.incrementAndGet();
          throw new IOException("fixture");
        },
        "$");
    assertEquals(2, calls.get());
  }

  @Test
  void missingTestEndpointsAreHintsButRealTestFetchFailuresStayUnknown() throws Exception {
    Report absent = new Report("keys", "TEST", "v3");
    CertificateRevocation.revocation(
        leaf(false), CryptoTest.issuer, absent, (url, post) -> fail("no endpoint"), "$");
    assertEquals("warning", absent.findings.getFirst().status());
    Report failed = new Report("keys", "TEST", "v3");
    CertificateRevocation.revocation(
        leaf(false, "https://example.org/crl"),
        CryptoTest.issuer,
        failed,
        (url, post) -> new PublicHttp.Response(500, Map.of(), new byte[0], URI.create(url)),
        "$");
    assertEquals("unknown", failed.findings.getFirst().status());
    assertTrue(failed.findings.getFirst().detail().contains("HTTP 500"));
  }

  @Test
  void responderCrlStatusDistinguishesGoodRevokedAndUnknown() throws Exception {
    X509Certificate signer = leaf(false, "https://example.org/crl");
    byte[] good = crl(false), revoked = crl(true);
    assertEquals(
        "confirmed",
        CertificateRevocation.responderCrlStatus(
            signer, CryptoTest.issuer, (url, post) -> response(url, good), CryptoTest.now));
    assertEquals(
        "error",
        CertificateRevocation.responderCrlStatus(
            signer, CryptoTest.issuer, (url, post) -> response(url, revoked), CryptoTest.now));
    assertEquals(
        "unknown",
        CertificateRevocation.responderCrlStatus(
            signer,
            CryptoTest.issuer,
            (url, post) -> {
              throw new IOException("fixture");
            },
            CryptoTest.now));
    assertEquals(
        "unknown",
        CertificateRevocation.responderCrlStatus(
            leaf(false), CryptoTest.issuer, (url, post) -> fail("no endpoint"), CryptoTest.now));
  }

  @Test
  void restrictedOrIndirectCrlDistributionPointsAreExcluded() {
    var uri =
        new GeneralNames(
            new GeneralName(GeneralName.uniformResourceIdentifier, "https://example.org/crl"));
    var restricted =
        new DistributionPoint(
            new DistributionPointName(uri), new ReasonFlags(ReasonFlags.keyCompromise), null);
    var indirect =
        new DistributionPoint(
            new DistributionPointName(uri),
            null,
            new GeneralNames(new GeneralName(new X500Name("CN=Other"))));
    assertTrue(
        CrlEndpoints.locations(new CRLDistPoint(new DistributionPoint[] {restricted, indirect}))
            .isEmpty());
    assertTrue(CrlEndpoints.locations(null).isEmpty());
  }
}
