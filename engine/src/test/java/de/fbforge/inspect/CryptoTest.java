package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.nimbusds.jose.*;
import com.nimbusds.jose.crypto.*;
import com.nimbusds.jose.jwk.*;
import com.nimbusds.jwt.*;
import java.math.BigInteger;
import java.security.*;
import java.security.cert.*;
import java.time.Instant;
import java.util.*;
import org.bouncycastle.asn1.x500.X500Name;
import org.bouncycastle.asn1.x509.*;
import org.bouncycastle.cert.*;
import org.bouncycastle.cert.jcajce.*;
import org.bouncycastle.cert.ocsp.*;
import org.bouncycastle.cert.ocsp.jcajce.JcaBasicOCSPRespBuilder;
import org.bouncycastle.operator.jcajce.*;
import org.junit.jupiter.api.*;

class CryptoTest {
  static KeyPair issuerKey, otherKey, leafKey;
  static X509Certificate issuer, leaf;
  static CertificateID id;
  static Instant now;

  @BeforeAll
  static void fixtures() throws Exception {
    now = Instant.now();
    KeyPairGenerator gen = KeyPairGenerator.getInstance("RSA");
    gen.initialize(2048);
    issuerKey = gen.generateKeyPair();
    otherKey = gen.generateKeyPair();
    leafKey = gen.generateKeyPair();
    issuer =
        certificate(
            "CN=Synthetic Test CA",
            issuerKey,
            "CN=Synthetic Test CA",
            issuerKey,
            BigInteger.ONE,
            true);
    leaf =
        certificate(
            "CN=Synthetic Leaf", leafKey, "CN=Synthetic Test CA", issuerKey, BigInteger.TWO, false);
    id =
        new CertificateID(
            new JcaDigestCalculatorProviderBuilder().build().get(CertificateID.HASH_SHA1),
            new JcaX509CertificateHolder(issuer),
            leaf.getSerialNumber());
  }

  static X509Certificate certificate(
      String subject,
      KeyPair subjectKey,
      String issuerName,
      KeyPair signer,
      BigInteger serial,
      boolean ca)
      throws Exception {
    var builder =
        new JcaX509v3CertificateBuilder(
            new X500Name(issuerName),
            serial,
            Date.from(now.minusSeconds(3600)),
            Date.from(now.plusSeconds(86400)),
            new X500Name(subject),
            subjectKey.getPublic());
    builder.addExtension(
        org.bouncycastle.asn1.x509.Extension.basicConstraints, true, new BasicConstraints(ca));
    builder.addExtension(
        org.bouncycastle.asn1.x509.Extension.keyUsage,
        true,
        new KeyUsage(
            ca
                ? KeyUsage.keyCertSign | KeyUsage.cRLSign
                : KeyUsage.digitalSignature | KeyUsage.nonRepudiation));
    return new JcaX509CertificateConverter()
        .getCertificate(
            builder.build(new JcaContentSignerBuilder("SHA256withRSA").build(signer.getPrivate())));
  }

  static byte[] ocsp(
      CertificateStatus status, KeyPair signer, CertificateID target, Instant from, Instant until)
      throws Exception {
    var digest = new JcaDigestCalculatorProviderBuilder().build().get(CertificateID.HASH_SHA1);
    var builder = new JcaBasicOCSPRespBuilder(issuerKey.getPublic(), digest);
    builder.addResponse(
        target, status, Date.from(from), until == null ? null : Date.from(until), null);
    var basic =
        builder.build(
            new JcaContentSignerBuilder("SHA256withRSA").build(signer.getPrivate()),
            new X509CertificateHolder[0],
            Date.from(now));
    return new OCSPRespBuilder().build(OCSPRespBuilder.SUCCESSFUL, basic).getEncoded();
  }

  static byte[] crl(boolean revoked, KeyPair signer, Instant start, Instant end) throws Exception {
    var builder = new X509v2CRLBuilder(new X500Name("CN=Synthetic Test CA"), Date.from(start));
    if (end != null) builder.setNextUpdate(Date.from(end));
    if (revoked) builder.addCRLEntry(leaf.getSerialNumber(), Date.from(now.minusSeconds(300)), 1);
    return builder
        .build(new JcaContentSignerBuilder("SHA256withRSA").build(signer.getPrivate()))
        .getEncoded();
  }

  @Test
  void verifiesGoodOcsp() throws Exception {
    assertEquals(
        "confirmed",
        Certificates.validateOcsp(
            ocsp(null, issuerKey, id, now.minusSeconds(60), now.plusSeconds(600)),
            id,
            issuer,
            now));
  }

  @Test
  void detectsRevokedOcsp() throws Exception {
    assertEquals(
        "error",
        Certificates.validateOcsp(
            ocsp(
                new RevokedStatus(Date.from(now.minusSeconds(120)), 1),
                issuerKey,
                id,
                now.minusSeconds(60),
                now.plusSeconds(600)),
            id,
            issuer,
            now));
  }

  @Test
  void unknownOcspDoesNotBecomeGood() throws Exception {
    assertEquals(
        "unknown",
        Certificates.validateOcsp(
            ocsp(new UnknownStatus(), issuerKey, id, now.minusSeconds(60), now.plusSeconds(600)),
            id,
            issuer,
            now));
  }

  @Test
  void rejectsForgedOcspSigner() throws Exception {
    assertEquals(
        "unknown",
        Certificates.validateOcsp(
            ocsp(null, otherKey, id, now.minusSeconds(60), now.plusSeconds(600)), id, issuer, now));
  }

  @Test
  void rejectsStaleAndUndatedOcsp() throws Exception {
    assertEquals(
        "unknown",
        Certificates.validateOcsp(
            ocsp(null, issuerKey, id, now.minusSeconds(172800), now.minusSeconds(86400)),
            id,
            issuer,
            now));
    assertEquals(
        "unknown",
        Certificates.validateOcsp(
            ocsp(null, issuerKey, id, now.minusSeconds(60), null), id, issuer, now));
  }

  @Test
  void rejectsWrongOcspCertificate() throws Exception {
    var other =
        new CertificateID(
            new JcaDigestCalculatorProviderBuilder().build().get(CertificateID.HASH_SHA1),
            new JcaX509CertificateHolder(issuer),
            BigInteger.TEN);
    assertEquals(
        "unknown",
        Certificates.validateOcsp(
            ocsp(null, issuerKey, other, now.minusSeconds(60), now.plusSeconds(600)),
            id,
            issuer,
            now));
  }

  @Test
  void verifiesGoodCrlAndDetectsRevocation() throws Exception {
    assertEquals(
        "confirmed",
        Certificates.validateCrl(
            crl(false, issuerKey, now.minusSeconds(60), now.plusSeconds(600)), leaf, issuer, now));
    assertEquals(
        "error",
        Certificates.validateCrl(
            crl(true, issuerKey, now.minusSeconds(60), now.plusSeconds(600)), leaf, issuer, now));
  }

  @Test
  void rejectsForgedOrStaleCrl() throws Exception {
    assertThrows(
        GeneralSecurityException.class,
        () ->
            Certificates.validateCrl(
                crl(false, otherKey, now.minusSeconds(60), now.plusSeconds(600)),
                leaf,
                issuer,
                now));
    assertEquals(
        "unknown",
        Certificates.validateCrl(
            crl(false, issuerKey, now.minusSeconds(1000), now.minusSeconds(600)),
            leaf,
            issuer,
            now));
  }

  @Test
  void signatureRequiresTrustedIssuerAndPinnedAlgorithm() throws Exception {
    var gen = KeyPairGenerator.getInstance("RSA");
    gen.initialize(4096);
    KeyPair pair = gen.generateKeyPair();
    RSAKey publicKey =
        new RSAKey.Builder((java.security.interfaces.RSAPublicKey) pair.getPublic())
            .keyID("test")
            .build();
    for (String issuer :
        List.of(
            "https://portal.auth-testing.fit-connect.fitko.dev", "https://attacker.example.org")) {
      SignedJWT jwt =
          new SignedJWT(
              new JWSHeader.Builder(JWSAlgorithm.PS512).keyID("test").build(),
              new JWTClaimsSet.Builder().issuer(issuer).build());
      jwt.sign(new RSASSASigner(pair.getPrivate()));
      assertEquals(
          issuer.contains("fit-connect.fitko.dev"), Signatures.verify(jwt, publicKey, "TEST"));
    }
    SignedJWT wrongAlg =
        new SignedJWT(
            new JWSHeader(JWSAlgorithm.RS256),
            new JWTClaimsSet.Builder()
                .issuer("https://portal.auth-testing.fit-connect.fitko.dev")
                .build());
    wrongAlg.sign(new RSASSASigner(pair.getPrivate()));
    assertFalse(Signatures.verify(wrongAlg, publicKey, "TEST"));
  }

  RSAKey leafJwk(java.security.PublicKey key) throws Exception {
    return new RSAKey.Builder((java.security.interfaces.RSAPublicKey) key)
        .algorithm(JWSAlgorithm.PS512)
        .keyID("synthetic-leaf")
        .keyOperations(Set.of(KeyOperation.VERIFY))
        .x509CertChain(
            List.of(
                com.nimbusds.jose.util.Base64.encode(leaf.getEncoded()),
                com.nimbusds.jose.util.Base64.encode(issuer.getEncoded())))
        .build();
  }

  static String pem(X509Certificate certificate) throws Exception {
    return "-----BEGIN CERTIFICATE-----\n"
        + Base64.getMimeEncoder(64, new byte[] {'\n'}).encodeToString(certificate.getEncoded())
        + "\n-----END CERTIFICATE-----\n";
  }

  @Test
  void absentCertificateUsageDoesNotScoreTheJwkInEitherEnvironment() throws Exception {
    var certificate =
        new JcaX509v3CertificateBuilder(
                new X500Name("CN=Public Test Certificate"),
                BigInteger.valueOf(17),
                Date.from(now.minusSeconds(60)),
                Date.from(now.plusSeconds(3600)),
                new X500Name("CN=Public Test Certificate"),
                leafKey.getPublic())
            .build(new JcaContentSignerBuilder("SHA256withRSA").build(leafKey.getPrivate()));
    var key =
        new RSAKey.Builder((java.security.interfaces.RSAPublicKey) leafKey.getPublic())
            .algorithm(JWEAlgorithm.RSA_OAEP_256)
            .keyOperations(Set.of(KeyOperation.WRAP_KEY))
            .keyID("test-encryption")
            .x509CertChain(List.of(com.nimbusds.jose.util.Base64.encode(certificate.getEncoded())))
            .build();
    for (String environment : List.of("TEST", "PROD")) {
      Report report = new Report("keys", environment, "v3");
      KeyDiagnostics.inspect(
          key.toJSONString(), report, new PublicHttp(report), new com.google.gson.JsonObject());
      assertNoCertificateUsageFindings(report);
      assertTrue(
          report.findings.stream()
              .anyMatch(
                  f -> f.title().equals("Schlüsseloperation") && f.status().equals("confirmed")));
      var summary = report.data.getAsJsonArray("keys").get(0).getAsJsonObject();
      assertEquals(
          key.toJSONObject().get("x5c").toString(),
          summary.getAsJsonObject("publicJwk").getAsJsonArray("x5c").asList().stream()
              .map(e -> e.getAsString())
              .toList()
              .toString());
      var cert = report.data.getAsJsonArray("certificates").get(0).getAsJsonObject();
      assertEquals(0, cert.get("chainIndex").getAsInt());
      assertFalse(cert.has("keyUsagePresent"));
      assertTrue(cert.get("pem").getAsString().contains("BEGIN CERTIFICATE"));
    }
  }

  @Test
  void certificateUsageDoesNotOverrideValidJwkOperations() throws Exception {
    var raw =
        com.google.gson.JsonParser.parseString(leafJwk(leafKey.getPublic()).toJSONString())
            .getAsJsonObject();
    raw.addProperty("alg", "RSA-OAEP-256");
    var ops = new com.google.gson.JsonArray();
    ops.add("wrapKey");
    raw.add("key_ops", ops);
    Report report = new Report("keys", "TEST", "v3");
    KeyDiagnostics.inspect(
        raw.toString(), report, new PublicHttp(report), new com.google.gson.JsonObject());
    assertNoCertificateUsageFindings(report);
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f -> f.title().equals("Schlüsseloperation") && f.status().equals("confirmed")));
  }

  @Test
  void certificateChainRemainsInspectableWithoutUsageScoring() throws Exception {
    Report report = new Report("keys", "TEST", "v3");
    Certificates.inspect(
        leafJwk(leafKey.getPublic()),
        report,
        new PublicHttp(report),
        new com.google.gson.JsonObject(),
        "$.keys[0]");
    assertNoCertificateUsageFindings(report);
    assertEquals(
        "Ausstellerzertifikat",
        report
            .data
            .getAsJsonArray("certificates")
            .get(1)
            .getAsJsonObject()
            .get("role")
            .getAsString());
  }

  static void assertNoCertificateUsageFindings(Report report) {
    assertFalse(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().contains("Key-Usage")
                        || f.title().contains("Zertifikatsverwendung")));
  }

  @Test
  void incompatibleJwkOperationsStillFailIndependentlyOfCertificateUsage() throws Exception {
    for (String alg : List.of("RSA-OAEP-256", "PS512")) {
      var raw =
          com.google.gson.JsonParser.parseString(leafJwk(leafKey.getPublic()).toJSONString())
              .getAsJsonObject();
      raw.addProperty("alg", alg);
      var ops = new com.google.gson.JsonArray();
      ops.add(alg.equals("PS512") ? "wrapKey" : "verify");
      raw.add("key_ops", ops);
      Report report = new Report("keys", "TEST", "v3");
      KeyDiagnostics.inspect(
          raw.toString(), report, new PublicHttp(report), new com.google.gson.JsonObject());
      assertTrue(
          report.findings.stream()
              .anyMatch(f -> f.title().equals("Schlüsseloperation") && f.status().equals("error")));
      assertNoCertificateUsageFindings(report);
    }
  }

  @Test
  void receivedRootDoesNotBecomeImplicitTrust() throws Exception {
    Report report = new Report("keys", "TEST", "v3");
    Certificates.inspect(
        leafJwk(leafKey.getPublic()),
        report,
        new PublicHttp(report),
        new com.google.gson.JsonObject(),
        "$.keys[0]");
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("Vertrauensanker nicht gewählt")
                        && f.status().equals("unknown")));
  }

  @Test
  void pkixUsesOnlyExplicitIndependentAnchor() throws Exception {
    var options = new com.google.gson.JsonObject();
    options.addProperty("trustPem", pem(issuer));
    Report trusted = new Report("keys", "TEST", "v3");
    Certificates.inspect(
        leafJwk(leafKey.getPublic()), trusted, new PublicHttp(trusted), options, "$.keys[0]");
    assertTrue(
        trusted.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("PKIX-Vertrauen gegen gewählte CA")
                        && f.status().equals("confirmed")));
    X509Certificate other =
        certificate(
            "CN=Synthetic Test CA",
            otherKey,
            "CN=Synthetic Test CA",
            otherKey,
            BigInteger.TEN,
            true);
    options.addProperty("trustPem", pem(other));
    Report untrusted = new Report("keys", "TEST", "v3");
    Certificates.inspect(
        leafJwk(leafKey.getPublic()), untrusted, new PublicHttp(untrusted), options, "$.keys[0]");
    assertTrue(
        untrusted.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("PKIX-Vertrauen gegen gewählte CA")
                        && f.status().equals("error")));
  }

  @Test
  void certificateMustMatchJwkPublicKey() throws Exception {
    Report report = new Report("keys", "TEST", "v3");
    var raw =
        com.google.gson.JsonParser.parseString(leafJwk(leafKey.getPublic()).toString())
            .getAsJsonObject();
    RSAKey different =
        new RSAKey.Builder((java.security.interfaces.RSAPublicKey) otherKey.getPublic()).build();
    raw.addProperty("n", different.getModulus().toString());
    raw.addProperty("e", different.getPublicExponent().toString());
    KeyDiagnostics.inspect(
        raw.toString(), report, new PublicHttp(report), new com.google.gson.JsonObject());
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f -> f.title().equals("Schlüssel nicht auswertbar") && f.status().equals("error")));
  }
}
