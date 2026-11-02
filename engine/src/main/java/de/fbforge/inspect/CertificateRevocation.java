package de.fbforge.inspect;

import static de.fbforge.inspect.Certificates.RFC;

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
import org.bouncycastle.asn1.x509.*;
import org.bouncycastle.cert.*;
import org.bouncycastle.cert.jcajce.*;
import org.bouncycastle.cert.ocsp.*;
import org.bouncycastle.operator.jcajce.*;

/** OCSP/CRL acquisition and verification. Chain trust is owned by Certificates. */
final class CertificateRevocation {
  static void revocation(
      X509Certificate certificate,
      X509Certificate issuer,
      Report report,
      PkiFetch http,
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
            var response =
                fetch(
                    http,
                    url,
                    new OCSPReqBuilder().addRequest(id).build().getEncoded(),
                    report,
                    path,
                    "OCSP");
            if (response == null) continue;
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
                "OCSP-Antwort erhalten (HTTP 200–299), aber Format, Signatur, Zuordnung oder"
                    + " Aktualität nicht nachgewiesen.",
                path,
                url);
          }
        }
      CRLDistPoint points = CRLDistPoint.fromExtensions(holder.getExtensions());
      for (String url : CrlEndpoints.locations(points)) {
        found = true;
        try {
          var response = fetch(http, url, null, report, path, "CRL");
          if (response == null) continue;
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
              "CRL erhalten (HTTP 200–299), aber Format, Signatur oder Aktualität nicht"
                  + " nachgewiesen.",
              path,
              url);
        }
      }
    } catch (Exception e) {
      report.unknown(
          "Sperrstatusdaten ungültig", "Zertifikatserweiterungen nicht auswertbar.", path, RFC);
    }
    if (!found) {
      if (report.environment.equals("TEST"))
        report.add(
            "warning",
            "Kein unterstützter Sperrendpunkt",
            "Dieses TEST-Zertifikat veröffentlicht keinen unterstützten OCSP-/CRL-Endpunkt. Das ist"
                + " im TEST-Betrieb zulässig; ein Sperrstatus wird nicht bestätigt.",
            path,
            RFC,
            "",
            "Kein öffentlicher Good-Nachweis.");
      else
        report.unknown(
            "Kein unterstützter Sperrendpunkt",
            "Kein vollständiger direkter HTTP(S)-CRL- oder OCSP-Endpunkt im Zertifikat.",
            path,
            RFC);
    }
  }

  private static PublicHttp.Response fetch(
      PkiFetch http, String url, byte[] post, Report report, String path, String kind) {
    try {
      var response = http.pki(url, post);
      if (response.ok()) return response;
      report.unknown(
          kind + "-Abruf nicht erfolgreich",
          "HTTP "
              + response.status()
              + ": Der öffentliche Endpunkt hat keine erfolgreiche Antwort geliefert. Ohne"
              + " Sperrauskunft ist keine kryptographische Statusprüfung möglich.",
          path,
          url);
    } catch (IOException failure) {
      report.unknown(
          kind + "-Abruf fehlgeschlagen",
          failure instanceof NetworkFailure known
              ? known.description()
              : "Die öffentliche Sperrauskunft konnte nicht vollständig abgerufen werden. Netzwerk"
                  + " und Abfrageprotokoll prüfen.",
          path,
          url);
    } catch (IllegalArgumentException unsupported) {
      report.unknown(
          kind + "-Abruf nicht zulässig",
          "Der veröffentlichte Endpunkt erfüllt die Regeln für öffentliche HTTP(S)-Ziele nicht.",
          path,
          url);
    }
    return null;
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
      X509Certificate signer, X509Certificate issuer, PkiFetch http, Instant now) {
    // No recursive OCSP requests: only bounded direct full CRLs can prove responder status.
    try {
      var points =
          CRLDistPoint.fromExtensions(new JcaX509CertificateHolder(signer).getExtensions());
      if (points == null) return "unknown";
      boolean good = false;
      for (String url : CrlEndpoints.locations(points)) {
        try {
          var response = http.pki(url, null);
          if (!response.ok()) continue;
          String status = validateCrl(response.body(), signer, issuer, now);
          if (status.equals("error")) return "error";
          good |= status.equals("confirmed");
        } catch (Exception ignored) {
          /* A failed endpoint cannot establish responder status. */
        }
      }
      return good ? "confirmed" : "unknown";
    } catch (Exception e) {
      return "unknown";
    }
  }

  private static boolean currentCrl(X509CRL crl, Instant now) {
    Date start = crl.getThisUpdate(), end = crl.getNextUpdate();
    // The issuer's signed nextUpdate defines the publication interval (V-PKI CA CRLs can exceed
    // seven days).
    return start != null
        && end != null
        && end.after(start)
        && !start.toInstant().isAfter(now.plusSeconds(300))
        && !end.toInstant().isBefore(now.minusSeconds(300));
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
        || !currentCrl(crl, now)) return "unknown";
    X509CRLEntry entry = crl.getRevokedCertificate(certificate.getSerialNumber());
    if (entry == null) return "confirmed";
    if (entry.hasUnsupportedCriticalExtension()
        || entry.getRevocationDate().toInstant().isAfter(now)) return "unknown";
    return "error";
  }
}
