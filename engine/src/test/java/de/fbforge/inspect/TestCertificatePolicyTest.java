package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.JsonObject;
import com.nimbusds.jose.JWEAlgorithm;
import com.nimbusds.jose.jwk.*;
import java.math.BigInteger;
import java.security.PublicKey;
import java.security.cert.*;
import java.time.Instant;
import java.util.*;
import org.bouncycastle.asn1.x500.X500Name;
import org.bouncycastle.cert.jcajce.*;
import org.bouncycastle.operator.jcajce.JcaContentSignerBuilder;
import org.junit.jupiter.api.*;

class TestCertificatePolicyTest {
  @BeforeAll
  static void fixtures() throws Exception {
    CryptoTest.fixtures();
  }

  private static X509Certificate selfSigned(boolean expired, boolean forged) throws Exception {
    Instant now = Instant.now();
    var builder =
        new JcaX509v3CertificateBuilder(
            new X500Name("CN=Synthetic self-signed test leaf"),
            BigInteger.valueOf(74),
            Date.from(now.minusSeconds(3600)),
            Date.from(expired ? now.minusSeconds(60) : now.plusSeconds(3600)),
            new X500Name("CN=Synthetic self-signed test leaf"),
            CryptoTest.leafKey.getPublic());
    byte[] encoded =
        builder
            .build(
                new JcaContentSignerBuilder("SHA256withRSA").build(CryptoTest.leafKey.getPrivate()))
            .getEncoded();
    if (forged) encoded[encoded.length - 1] ^= 1;
    return new JcaX509CertificateConverter()
        .getCertificate(new org.bouncycastle.cert.X509CertificateHolder(encoded));
  }

  private static Report inspect(
      String environment, X509Certificate certificate, PublicKey publicKey) throws Exception {
    var builder =
        new RSAKey.Builder((java.security.interfaces.RSAPublicKey) publicKey)
            .keyID("synthetic-test-encryption")
            .algorithm(JWEAlgorithm.RSA_OAEP_256)
            .keyOperations(Set.of(KeyOperation.WRAP_KEY));
    if (certificate != null)
      builder.x509CertChain(
          List.of(com.nimbusds.jose.util.Base64.encode(certificate.getEncoded())));
    Report report = new Report("keys", environment, "v3");
    Certificates.inspect(
        builder.build(),
        report,
        (url, post) -> fail("No implicit network call without an endpoint"),
        new JsonObject(),
        "$.keys[0]");
    return report;
  }

  @Test
  void absentX5cIsOnlyAHintInTestAndStillAnErrorInProd() throws Exception {
    for (String environment : List.of("TEST", "STAGE", "PROD")) {
      Report report = inspect(environment, null, CryptoTest.leafKey.getPublic());
      assertEquals(
          environment.equals("TEST") ? "warning" : "error", report.findings.getFirst().status());
    }
  }

  @Test
  void selfSignedTestCertificateIsAHintWithoutClaimingVpkiTrustOrRevocation() throws Exception {
    Report report = inspect("TEST", selfSigned(false, false), CryptoTest.leafKey.getPublic());
    assertTrue(
        report.findings.stream()
            .noneMatch(f -> f.status().equals("error") || f.status().equals("unknown")));
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("TEST-Zertifikat ohne V-PKI-Verankerung")
                        && f.status().equals("warning")));
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("Sperrstatus des TEST-Zertifikats")
                        && f.status().equals("warning")));
    assertFalse(
        report
            .data
            .getAsJsonArray("certificateTrust")
            .get(0)
            .getAsJsonObject()
            .get("trusted")
            .getAsBoolean());
  }

  @Test
  void sameSelfSignedCertificateRemainsUntrustedOutsideTest() throws Exception {
    var certificate = selfSigned(false, false);
    for (String environment : List.of("STAGE", "PROD")) {
      Report report = inspect(environment, certificate, CryptoTest.leafKey.getPublic());
      assertTrue(
          report.findings.stream()
              .anyMatch(
                  f -> f.title().equals("V-PKI-Vertrauenskette") && f.status().equals("error")));
    }
  }

  @Test
  void forgedSelfSignatureStillFailsInTest() throws Exception {
    Report report = inspect("TEST", selfSigned(false, true), CryptoTest.leafKey.getPublic());
    assertTrue(
        report.findings.stream()
            .anyMatch(f -> f.title().equals("Kettensignatur") && f.status().equals("error")));
  }

  @Test
  void expiredTestCertificateStillFails() throws Exception {
    Report report = inspect("TEST", selfSigned(true, false), CryptoTest.leafKey.getPublic());
    assertTrue(
        report.findings.stream()
            .anyMatch(f -> f.title().equals("Zertifikatslaufzeit") && f.status().equals("error")));
  }
}
