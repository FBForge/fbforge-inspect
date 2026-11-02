package de.fbforge.inspect;

import com.google.gson.*;
import com.nimbusds.jose.jwk.*;
import java.io.*;
import java.security.*;
import java.security.cert.*;
import java.time.*;
import java.util.*;
import org.bouncycastle.asn1.ASN1Primitive;
import org.bouncycastle.asn1.x509.*;
import org.bouncycastle.cert.*;
import org.bouncycastle.cert.jcajce.*;
import org.bouncycastle.cert.ocsp.*;
import org.bouncycastle.jce.provider.BouncyCastleProvider;
import org.bouncycastle.operator.jcajce.*;

/** Trust, chain integrity and revocation are deliberately independent findings. */
final class Certificates {
  static {
    if (Security.getProvider("BC") == null) Security.addProvider(new BouncyCastleProvider());
  }

  static final String RFC = "https://www.rfc-editor.org/rfc/rfc6960";

  static void inspect(JWK key, Report r, PkiFetch http, JsonObject options, String path)
      throws Exception {
    var x5c = key.getX509CertChain();
    if (x5c == null || x5c.isEmpty()) {
      r.add(
          r.environment.equals("TEST") ? "warning" : "error",
          "Zertifikatskette fehlt",
          "TEST erlaubt Testschlüssel. Ohne x5c kein Produktionsnachweis.",
          path + ".x5c",
          KeyDiagnostics.SOURCE,
          "Zertifikatskette ergänzen.",
          "");
      return;
    }
    if (x5c.size() > 10) throw new IllegalArgumentException("Zertifikatskette zu lang.");
    CertificateFactory factory = CertificateFactory.getInstance("X.509");
    List<X509Certificate> chain = new ArrayList<>();
    for (var cert : x5c)
      chain.add(
          (X509Certificate) factory.generateCertificate(new ByteArrayInputStream(cert.decode())));
    X509Certificate leaf = chain.getFirst();
    PublicKey publicKey =
        key instanceof RSAKey rsa ? rsa.toRSAPublicKey() : key.toECKey().toECPublicKey();
    r.check(
        Arrays.equals(publicKey.getEncoded(), leaf.getPublicKey().getEncoded()),
        "JWK und Zertifikat",
        "SPKI des ersten x5c-Zertifikats muss zum JWK passen.",
        path + ".x5c[0]",
        KeyDiagnostics.SOURCE);
    JsonArray certificates = new JsonArray();
    for (int i = 0; i < chain.size(); i++) {
      X509Certificate c = chain.get(i);
      String p = path + ".x5c[" + i + "]";
      JsonObject info = new JsonObject();
      info.addProperty("keyPath", path);
      info.addProperty("kid", key.getKeyID());
      info.addProperty("chainIndex", i);
      info.addProperty("role", i == 0 ? "Teilnehmerzertifikat" : "Ausstellerzertifikat");
      info.addProperty("x5c", x5c.get(i).toString());
      info.addProperty(
          "pem",
          "-----BEGIN CERTIFICATE-----\n"
              + Base64.getMimeEncoder(64, new byte[] {'\n'}).encodeToString(c.getEncoded())
              + "\n-----END CERTIFICATE-----");
      info.addProperty("subject", c.getSubjectX500Principal().getName());
      info.addProperty("issuer", c.getIssuerX500Principal().getName());
      info.addProperty("notBefore", c.getNotBefore().toInstant().toString());
      info.addProperty("notAfter", c.getNotAfter().toInstant().toString());
      info.addProperty(
          "sha256",
          HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(c.getEncoded())));
      certificates.add(info);
      boolean valid = true;
      try {
        c.checkValidity();
      } catch (CertificateException e) {
        valid = false;
      }
      r.check(
          valid,
          "Zertifikatslaufzeit",
          c.getSubjectX500Principal()
              + " | "
              + c.getNotBefore().toInstant()
              + " bis "
              + c.getNotAfter().toInstant(),
          p,
          KeyDiagnostics.SOURCE);
      if (i + 1 < chain.size()) {
        boolean signature = true;
        try {
          c.verify(chain.get(i + 1).getPublicKey());
          if (!c.getIssuerX500Principal().equals(chain.get(i + 1).getSubjectX500Principal()))
            signature = false;
        } catch (Exception e) {
          signature = false;
        }
        r.check(
            signature,
            "Kettensignatur",
            "Signatur mit dem folgenden Ausstellerzertifikat.",
            p,
            KeyDiagnostics.SOURCE);
      }
    }
    if (!r.data.has("certificates")) r.data.add("certificates", new JsonArray());
    r.data.getAsJsonArray("certificates").addAll(certificates);
    // Participant-key usage is assessed only from the public JWK fields.
    // X.509 Key Usage is not scored as a FIT-Connect JWK acceptance criterion.
    boolean pss = leaf.getSigAlgOID().equals("1.2.840.113549.1.1.10"), sha512 = false;
    if (pss && leaf.getSigAlgParams() != null) {
      var parameters =
          org.bouncycastle.asn1.pkcs.RSASSAPSSparams.getInstance(
              ASN1Primitive.fromByteArray(leaf.getSigAlgParams()));
      var mgf = parameters.getMaskGenAlgorithm();
      sha512 =
          parameters.getHashAlgorithm().getAlgorithm().getId().equals("2.16.840.1.101.3.4.2.3")
              && mgf.getAlgorithm().getId().equals("1.2.840.113549.1.1.8")
              && AlgorithmIdentifier.getInstance(mgf.getParameters())
                  .getAlgorithm()
                  .getId()
                  .equals("2.16.840.1.101.3.4.2.3")
              && parameters.getSaltLength().intValueExact() == 64
              && parameters.getTrailerField().intValueExact() == 1;
    }
    r.add(
        pss && sha512 ? "confirmed" : r.environment.equals("TEST") ? "warning" : "error",
        "Zertifikatsalgorithmus",
        "FITKO-Profil: RSASSA-PSS mit SHA-512.",
        path + ".x5c[0]",
        KeyDiagnostics.SOURCE,
        "",
        r.environment.equals("TEST") ? "TEST-Profil beweist keine Produktionstauglichkeit." : "");
    String pem = JsonFields.value(options, "trustPem");
    if (pem.isBlank())
      r.unknown(
          FindingCode.TRUST_ANCHOR_MISSING,
          "Vertrauensanker nicht gewählt",
          "Die x5c-Wurzel wird nicht automatisch vertraut. Öffentliche CA-Zertifikate aus einer"
              + " unabhängigen Quelle laden.",
          path,
          KeyDiagnostics.SOURCE);
    else {
      Set<TrustAnchor> anchors = new HashSet<>();
      for (var c :
          factory.generateCertificates(
              new ByteArrayInputStream(pem.getBytes(java.nio.charset.StandardCharsets.US_ASCII)))) {
        X509Certificate root = (X509Certificate) c;
        root.checkValidity();
        root.verify(root.getPublicKey());
        if (root.getBasicConstraints() < 0) throw new CertificateException("Kein CA-Anker.");
        anchors.add(new TrustAnchor(root, null));
      }
      if (anchors.isEmpty() || anchors.size() > 16)
        throw new CertificateException("Keine oder zu viele Vertrauensanker.");
      List<X509Certificate> pathCerts = new ArrayList<>(chain);
      if (anchors.stream().anyMatch(a -> a.getTrustedCert().equals(pathCerts.getLast())))
        pathCerts.removeLast();
      PKIXParameters params = new PKIXParameters(anchors);
      params.setRevocationEnabled(false);
      boolean trusted = true;
      try {
        CertPathValidator.getInstance("PKIX").validate(factory.generateCertPath(pathCerts), params);
      } catch (GeneralSecurityException e) {
        trusted = false;
      }
      r.check(
          trusted,
          "PKIX-Vertrauen gegen gewählte CA",
          "Nur explizit geladene CA-Anker. Deren V-PKI-Zugehörigkeit muss unabhängig feststehen.",
          path,
          KeyDiagnostics.SOURCE);
    }
    boolean online = options.has("revocation") && options.get("revocation").getAsBoolean();
    if (!online) {
      r.unknown(
          FindingCode.REVOCATION_NOT_REQUESTED,
          "Sperrstatus nicht abgefragt",
          "Onlineprüfung benötigt die ausdrückliche Auswahl von OCSP/CRL.",
          path,
          RFC);
      return;
    }
    if (chain.size() < 2) {
      r.unknown(
          "Sperrstatus ohne Aussteller", "Kein Ausstellerzertifikat in x5c vorhanden.", path, RFC);
      return;
    }
    for (int i = 0; i < chain.size() - 1; i++)
      CertificateRevocation.revocation(
          chain.get(i), chain.get(i + 1), r, http, path + ".x5c[" + i + "]");
  }
}
