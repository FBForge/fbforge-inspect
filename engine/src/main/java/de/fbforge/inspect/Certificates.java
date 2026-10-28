package de.fbforge.inspect;

import com.google.gson.*;
import com.nimbusds.jose.jwk.*;
import java.io.*;
import java.security.*;
import java.security.cert.*;
import java.time.*;
import java.util.*;
import java.util.function.Function;
import org.bouncycastle.asn1.ASN1Null;
import org.bouncycastle.asn1.ASN1ObjectIdentifier;
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

  static void inspect(JWK key, Report r, PublicHttp http, JsonObject options, String path)
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
    boolean[] usage = leaf.getKeyUsage();
    boolean signing = key.getAlgorithm() != null && key.getAlgorithm().getName().equals("PS512");
    r.check(
        usage != null
            && (signing ? usage.length > 1 && usage[0] && usage[1] : usage.length > 2 && usage[2]),
        "Zertifikatsverwendung",
        signing ? "digitalSignature und nonRepudiation erwartet." : "keyEncipherment erwartet.",
        path + ".x5c[0]",
        KeyDiagnostics.SOURCE);
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
    String pem = Api.value(options, "trustPem");
    if (pem.isBlank())
      r.unknown(
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
      revocation(chain.get(i), chain.get(i + 1), r, http, path + ".x5c[" + i + "]");
  }

  static void revocation(
      X509Certificate certificate,
      X509Certificate issuer,
      Report report,
      PublicHttp http,
      String path) {
    boolean found = false;
    try {
      X509CertificateHolder holder = new JcaX509CertificateHolder(certificate);
      AuthorityInformationAccess aia =
          AuthorityInformationAccess.fromExtensions(holder.getExtensions());
      if (aia != null)
        for (AccessDescription access : aia.getAccessDescriptions()) {
          if (!access.getAccessMethod().equals(AccessDescription.id_ad_ocsp)
              || access.getAccessLocation().getTagNo() != GeneralName.uniformResourceIdentifier)
            continue;
          if (found) break;
          found = true;
          String url = access.getAccessLocation().getName().toString();
          CertificateID id =
              new CertificateID(
                  new JcaDigestCalculatorProviderBuilder().build().get(CertificateID.HASH_SHA1),
                  new JcaX509CertificateHolder(issuer),
                  certificate.getSerialNumber());
          try {
            var response = http.pki(url, new OCSPReqBuilder().addRequest(id).build().getEncoded());
            if (!response.ok()) throw new IOException();
            Instant now = Instant.now();
            String status =
                validateOcsp(
                    response.body(),
                    id,
                    issuer,
                    now,
                    signer -> responderCrlStatus(signer, issuer, http, now));
            report.add(
                status,
                "OCSP-Sperrstatus",
                status.equals("confirmed")
                    ? "Signierte aktuelle Good-Antwort für dieses Zertifikat."
                    : status.equals("error")
                        ? "Zertifikat laut signierter aktueller OCSP-Antwort gesperrt."
                        : "Sperrstatus unbekannt oder Antwort nicht verifizierbar.",
                path,
                url,
                "",
                status.equals("unknown") ? "Erreichbarkeit ist kein Sperrstatusnachweis." : "");
          } catch (Exception e) {
            report.unknown(
                "OCSP nicht verifizierbar",
                "Abruf, Signatur, Zuordnung oder Aktualität nicht nachgewiesen.",
                path,
                url);
          }
        }
      CRLDistPoint points = CRLDistPoint.fromExtensions(holder.getExtensions());
      int attempted = 0;
      if (points != null)
        for (DistributionPoint dp : points.getDistributionPoints()) {
          if (dp.getReasons() != null
              || dp.getCRLIssuer() != null
              || dp.getDistributionPoint() == null
              || dp.getDistributionPoint().getType() != DistributionPointName.FULL_NAME) continue;
          for (GeneralName name :
              GeneralNames.getInstance(dp.getDistributionPoint().getName()).getNames()) {
            if (name.getTagNo() != GeneralName.uniformResourceIdentifier || attempted++ >= 2)
              continue;
            found = true;
            String url = name.getName().toString();
            try {
              var response = http.pki(url, null);
              if (!response.ok()) throw new IOException();
              String status = validateCrl(response.body(), certificate, issuer, Instant.now());
              report.add(
                  status,
                  "CRL-Sperrstatus",
                  status.equals("confirmed")
                      ? "Aktuelle vollständige CRL signiert, Seriennummer nicht gesperrt."
                      : status.equals("error")
                          ? "Zertifikat in signierter aktueller CRL gesperrt."
                          : "CRL-Scope, Signatur oder Aktualität nicht nachgewiesen.",
                  path,
                  url,
                  "",
                  status.equals("unknown")
                      ? "Delta- und indirekte CRLs gelten nicht als Good-Nachweis."
                      : "");
            } catch (Exception e) {
              report.unknown(
                  "CRL nicht verifizierbar",
                  "Abruf, Signatur oder Aktualität nicht nachgewiesen.",
                  path,
                  url);
            }
          }
        }
    } catch (Exception e) {
      report.unknown(
          "Sperrstatusdaten ungültig", "Zertifikatserweiterungen nicht auswertbar.", path, RFC);
    }
    if (!found)
      report.unknown(
          "Kein unterstützter Sperrendpunkt",
          "Kein vollständiger direkter HTTP(S)-CRL- oder OCSP-Endpunkt im Zertifikat.",
          path,
          RFC);
  }

  static boolean fresh(Date start, Date end, Instant now, Duration maxAge) {
    return start != null
        && end != null
        && !start.toInstant().isAfter(now.plusSeconds(300))
        && end.after(start)
        && !end.toInstant().isBefore(now.minusSeconds(300))
        && !start.toInstant().isBefore(now.minus(maxAge));
  }

  static String validateOcsp(byte[] encoded, CertificateID id, X509Certificate issuer, Instant now)
      throws Exception {
    return validateOcsp(encoded, id, issuer, now, signer -> "unknown");
  }

  static String validateOcsp(
      byte[] encoded,
      CertificateID id,
      X509Certificate issuer,
      Instant now,
      Function<X509Certificate, String> responderStatus)
      throws Exception {
    OCSPResp response = new OCSPResp(encoded);
    if (response.getStatus() != 0 || !(response.getResponseObject() instanceof BasicOCSPResp basic))
      return "unknown";
    if (basic.getCriticalExtensionOIDs() != null && !basic.getCriticalExtensionOIDs().isEmpty())
      return "unknown";
    List<X509Certificate> signers = new ArrayList<>();
    signers.add(issuer);
    for (X509CertificateHolder c : basic.getCerts())
      signers.add(new JcaX509CertificateConverter().getCertificate(c));
    boolean valid = false;
    for (X509Certificate signer : signers) {
      try {
        signer.checkValidity(Date.from(now));
        if (!signer.equals(issuer)) {
          signer.verify(issuer.getPublicKey());
          if (!signer.getIssuerX500Principal().equals(issuer.getSubjectX500Principal())
              || signer.getExtendedKeyUsage() == null
              || !signer.getExtendedKeyUsage().contains("1.3.6.1.5.5.7.3.9")) continue;
          boolean[] ku = signer.getKeyUsage();
          if (ku != null && !ku[0]) continue;
          if (signer.hasUnsupportedCriticalExtension()) continue;
        }
        var provider = new JcaDigestCalculatorProviderBuilder().build();
        var byKey =
            new RespID(
                new JcaX509CertificateHolder(signer).getSubjectPublicKeyInfo(),
                provider.get(RespID.HASH_SHA1));
        var byName = new RespID(new JcaX509CertificateHolder(signer).getSubject());
        if (!(basic.getResponderId().equals(byKey) || basic.getResponderId().equals(byName)))
          continue;
        if (basic.isSignatureValid(
            new JcaContentVerifierProviderBuilder().build(signer.getPublicKey()))) {
          // Authorization alone does not establish the delegated signer's current status.
          if (!signer.equals(issuer)
              && !hasNoCheck(signer)
              && !"confirmed".equals(responderStatus.apply(signer))) continue;
          valid = true;
          break;
        }
      } catch (Exception ignored) {
        /* Try another authorized response signer. */
      }
    }
    if (!valid
        || basic.getProducedAt() == null
        || basic.getProducedAt().toInstant().isAfter(now.plusSeconds(300))
        || basic.getProducedAt().toInstant().isBefore(now.minus(Duration.ofDays(1))))
      return "unknown";
    SingleResp matched = null;
    for (SingleResp single : basic.getResponses())
      if (single.getCertID().equals(id)) {
        if (matched != null) return "unknown";
        matched = single;
      }
    if (matched == null
        || !fresh(matched.getThisUpdate(), matched.getNextUpdate(), now, Duration.ofDays(1))
        || !matched.getCriticalExtensionOIDs().isEmpty()) return "unknown";
    CertificateStatus status = matched.getCertStatus();
    if (status == CertificateStatus.GOOD) return "confirmed";
    return status instanceof RevokedStatus revoked
            && !revoked.getRevocationTime().toInstant().isAfter(now)
        ? "error"
        : "unknown";
  }

  static boolean hasNoCheck(X509Certificate signer) {
    try {
      // Some JCA providers normalize nocheck to empty bytes. Inspect the signed DER instead.
      var extension =
          new JcaX509CertificateHolder(signer)
              .getExtension(new ASN1ObjectIdentifier("1.3.6.1.5.5.7.48.1.5"));
      return extension != null && extension.getParsedValue() instanceof ASN1Null;
    } catch (Exception e) {
      return false;
    }
  }

  static String responderCrlStatus(
      X509Certificate signer, X509Certificate issuer, PublicHttp http, Instant now) {
    // No recursive OCSP requests: only bounded direct full CRLs can prove responder status.
    try {
      var points =
          CRLDistPoint.fromExtensions(new JcaX509CertificateHolder(signer).getExtensions());
      if (points == null) return "unknown";
      int attempted = 0;
      boolean good = false;
      for (DistributionPoint dp : points.getDistributionPoints()) {
        if (dp.getReasons() != null
            || dp.getCRLIssuer() != null
            || dp.getDistributionPoint() == null
            || dp.getDistributionPoint().getType() != DistributionPointName.FULL_NAME) continue;
        for (GeneralName name :
            GeneralNames.getInstance(dp.getDistributionPoint().getName()).getNames()) {
          if (name.getTagNo() != GeneralName.uniformResourceIdentifier || attempted++ >= 2)
            continue;
          try {
            var response = http.pki(name.getName().toString(), null);
            if (!response.ok()) continue;
            String status = validateCrl(response.body(), signer, issuer, now);
            if (status.equals("error")) return "error";
            good |= status.equals("confirmed");
          } catch (Exception ignored) {
            /* A failed endpoint cannot establish responder status. */
          }
        }
      }
      return good ? "confirmed" : "unknown";
    } catch (Exception e) {
      return "unknown";
    }
  }

  static String validateCrl(
      byte[] encoded, X509Certificate certificate, X509Certificate issuer, Instant now)
      throws Exception {
    X509CRL crl =
        (X509CRL)
            CertificateFactory.getInstance("X.509").generateCRL(new ByteArrayInputStream(encoded));
    if (!crl.getIssuerX500Principal().equals(issuer.getSubjectX500Principal())
        || !certificate.getIssuerX500Principal().equals(issuer.getSubjectX500Principal()))
      return "unknown";
    boolean[] ku = issuer.getKeyUsage();
    if (ku == null || ku.length <= 6 || !ku[6]) return "unknown";
    issuer.checkValidity(Date.from(now));
    crl.verify(issuer.getPublicKey());
    if (crl.getExtensionValue("2.5.29.27") != null
        || crl.getExtensionValue("2.5.29.28") != null
        || crl.hasUnsupportedCriticalExtension()
        || !fresh(crl.getThisUpdate(), crl.getNextUpdate(), now, Duration.ofDays(7)))
      return "unknown";
    X509CRLEntry entry = crl.getRevokedCertificate(certificate.getSerialNumber());
    if (entry == null) return "confirmed";
    if (entry.hasUnsupportedCriticalExtension()
        || entry.getRevocationDate().toInstant().isAfter(now)) return "unknown";
    return "error";
  }
}
