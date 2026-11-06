package de.fbforge.inspect;

import com.google.gson.*;
import com.nimbusds.jose.jwk.*;
import java.io.*;
import java.security.*;
import java.security.cert.*;
import java.time.*;
import java.util.*;
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
          FindingCode.CERTIFICATE_CHAIN,
          r.environment.equals("TEST") ? "warning" : "error",
          "Zertifikatskette fehlt",
          "TEST erlaubt Testschlüssel. Ohne x5c kein Produktionsnachweis.",
          path + ".x5c",
          KeyDiagnostics.SOURCE,
          r.environment.equals("TEST") ? "" : "Zertifikatskette ergänzen.",
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
        FindingCode.CERTIFICATE_BINDING,
        Arrays.equals(publicKey.getEncoded(), leaf.getPublicKey().getEncoded()),
        "JWK und Zertifikat",
        "SPKI des ersten x5c-Zertifikats muss zum JWK passen.",
        path + ".x5c[0]",
        KeyDiagnostics.SOURCE);
    inspectChain(chain, r, http, options, path, key.getKeyID());
  }

  static void inspectChain(
      List<X509Certificate> chain,
      Report r,
      PkiFetch http,
      JsonObject options,
      String path,
      String kid)
      throws Exception {
    X509Certificate leaf = chain.getFirst();
    JsonArray certificates = new JsonArray();
    for (int i = 0; i < chain.size(); i++) {
      X509Certificate c = chain.get(i);
      String p = path + ".x5c[" + i + "]";
      JsonObject info = new JsonObject();
      info.addProperty("keyPath", path);
      info.addProperty("kid", kid);
      info.addProperty("chainIndex", i);
      info.addProperty("role", i == 0 ? "Teilnehmerzertifikat" : "Ausstellerzertifikat");
      info.addProperty("x5c", Base64.getEncoder().encodeToString(c.getEncoded()));
      info.addProperty(
          "pem",
          "-----BEGIN CERTIFICATE-----\n"
              + Base64.getMimeEncoder(64, new byte[] {'\n'}).encodeToString(c.getEncoded())
              + "\n-----END CERTIFICATE-----");
      info.addProperty("subject", c.getSubjectX500Principal().getName());
      info.addProperty("issuer", c.getIssuerX500Principal().getName());
      info.addProperty("signatureAlgorithm", c.getSigAlgName());
      info.addProperty("signatureAlgorithmOid", c.getSigAlgOID());
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
          FindingCode.CERTIFICATE_VALIDITY,
          valid,
          "Zertifikatslaufzeit",
          c.getSubjectX500Principal()
              + " | "
              + c.getNotBefore().toInstant()
              + " bis "
              + c.getNotAfter().toInstant(),
          p,
          KeyDiagnostics.SOURCE);
      X509Certificate signer =
          i + 1 < chain.size()
              ? chain.get(i + 1)
              : c.getIssuerX500Principal().equals(c.getSubjectX500Principal()) ? c : null;
      if (signer != null) {
        boolean signature = true;
        try {
          c.verify(signer.getPublicKey());
          if (!c.getIssuerX500Principal().equals(signer.getSubjectX500Principal()))
            signature = false;
        } catch (Exception e) {
          signature = false;
        }
        r.check(
            FindingCode.CERTIFICATE_SIGNATURE,
            signature,
            "Kettensignatur",
            signer == c
                ? "Selbstsignatur des Zertifikats."
                : "Signatur mit dem folgenden Ausstellerzertifikat.",
            p,
            KeyDiagnostics.SOURCE);
      }
    }
    if (!r.data.has("certificates")) r.data.add("certificates", new JsonArray());
    r.data.getAsJsonArray("certificates").addAll(certificates);
    // JWK operations are checked in KeyDiagnostics. Certificate signature algorithms are
    // displayed as certificate evidence, not scored as a JWK algorithm requirement.
    List<X509Certificate> resolved = CertificateTrust.inspect(chain, options, r, path);
    boolean online = !options.has("revocation") || options.get("revocation").getAsBoolean();
    if (!online) {
      r.unknown(
          FindingCode.REVOCATION_NOT_REQUESTED,
          "Sperrstatus nicht abgefragt",
          "Die öffentliche OCSP-/CRL-Prüfung wurde für diesen Lauf ausgeschaltet.",
          path,
          RFC);
      return;
    }
    if (resolved.size() < 2) {
      if (r.environment.equals("TEST")
          && leaf.getSubjectX500Principal().equals(leaf.getIssuerX500Principal())) {
        r.add(
            FindingCode.REVOCATION_UNAVAILABLE,
            "warning",
            "Sperrstatus des TEST-Zertifikats",
            "Für dieses selbstsignierte TEST-Zertifikat ist kein unabhängiger Aussteller verfügbar."
                + " Im TEST-Betrieb ist das zulässig. Ein öffentlicher Sperrstatus wird nicht"
                + " bestätigt.",
            path + ".x5c[0]",
            RFC,
            "",
            "Keine Produktionseignung und kein Good-Nachweis aus der Selbstsignatur.");
        return;
      }
      r.unknown(
          FindingCode.REVOCATION_UNAVAILABLE,
          "Sperrstatus ohne Aussteller",
          "Kein Ausstellerzertifikat in x5c vorhanden.",
          path,
          RFC);
      return;
    }
    for (int i = 0; i < resolved.size() - 1; i++) {
      int originalIndex = chain.indexOf(resolved.get(i));
      String certificatePath =
          originalIndex >= 0
              ? path + ".x5c[" + originalIndex + "]"
              : "$.certificateTrust["
                  + (r.data.getAsJsonArray("certificateTrust").size() - 1)
                  + "].chain["
                  + i
                  + "]";
      CertificateRevocation.revocation(
          resolved.get(i), resolved.get(i + 1), r, http, certificatePath);
    }
  }
}
