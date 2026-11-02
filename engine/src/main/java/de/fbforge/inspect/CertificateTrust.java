package de.fbforge.inspect;

import com.google.gson.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.security.cert.*;
import java.time.*;
import java.util.*;

/** PKIX builds only to independent bundled roots or an explicit additional user anchor. */
final class CertificateTrust {
  private CertificateTrust() {}

  static List<X509Certificate> inspect(
      List<X509Certificate> chain, JsonObject options, Report report, String path)
      throws GeneralSecurityException, IOException {
    return inspect(chain, options, report, path, Instant.now());
  }

  static List<X509Certificate> inspect(
      List<X509Certificate> chain, JsonObject options, Report report, String path, Instant now)
      throws GeneralSecurityException, IOException {
    List<VpkiTrustStore.Entry> entries = VpkiTrustStore.validEntries(now);
    Set<TrustAnchor> anchors = new HashSet<>();
    List<X509Certificate> material = new ArrayList<>(chain);
    Set<String> bundled = new HashSet<>();
    for (VpkiTrustStore.Entry entry : entries) {
      material.add(entry.certificate());
      if (entry.root()) {
        anchors.add(new TrustAnchor(entry.certificate(), null));
        bundled.add(entry.fingerprint());
      }
    }
    String pem = JsonFields.value(options, "trustPem");
    CertificateFactory factory = CertificateFactory.getInstance("X.509");
    if (!pem.isBlank()) {
      Collection<? extends java.security.cert.Certificate> extra =
          factory.generateCertificates(
              new ByteArrayInputStream(pem.getBytes(StandardCharsets.US_ASCII)));
      if (extra.isEmpty() || extra.size() > 16)
        throw new CertificateException("Keine oder zu viele zusätzliche CA-Anker.");
      for (var value : extra) {
        X509Certificate root = (X509Certificate) value;
        root.checkValidity(Date.from(now));
        root.verify(root.getPublicKey());
        if (root.getBasicConstraints() < 0
            || !root.getSubjectX500Principal().equals(root.getIssuerX500Principal()))
          throw new CertificateException("Kein selbstsignierter CA-Anker.");
        anchors.add(new TrustAnchor(root, null));
      }
    }
    if (anchors.isEmpty()) {
      report.unknown(
          FindingCode.TRUST_ANCHOR_MISSING,
          "Kein gültiger Vertrauensanker",
          "Der mitgelieferte V-PKI-Truststore enthält derzeit keine gültige Wurzel.",
          path,
          "https://www.bsi.bund.de/VPKI");
      return chain;
    }
    JsonObject evidence = new JsonObject();
    evidence.addProperty("keyPath", path);
    evidence.addProperty("storeAsOf", VpkiTrustStore.asOf());
    evidence.addProperty("bundledRoots", bundled.size());
    evidence.addProperty("additionalRoots", anchors.size() - bundled.size());
    if (!report.data.has("certificateTrust")) report.data.add("certificateTrust", new JsonArray());
    report.data.getAsJsonArray("certificateTrust").add(evidence);
    X509CertSelector selector = new X509CertSelector();
    selector.setCertificate(chain.getFirst());
    PKIXBuilderParameters parameters = new PKIXBuilderParameters(anchors, selector);
    parameters.setDate(Date.from(now));
    parameters.setRevocationEnabled(false);
    parameters.addCertStore(
        CertStore.getInstance("Collection", new CollectionCertStoreParameters(material)));
    try {
      PKIXCertPathBuilderResult result =
          (PKIXCertPathBuilderResult) CertPathBuilder.getInstance("PKIX").build(parameters);
      X509Certificate root = result.getTrustAnchor().getTrustedCert();
      String fingerprint = VpkiTrustStore.fingerprint(root);
      boolean vpki = bundled.contains(fingerprint);
      evidence.addProperty("trusted", true);
      evidence.addProperty("source", vpki ? "bundled-vpki" : "additional-ca");
      evidence.addProperty("anchor", root.getSubjectX500Principal().getName());
      evidence.addProperty("anchorSha256", fingerprint);
      report.check(
          true,
          pem.isBlank() ? "V-PKI-Vertrauenskette" : "PKIX-Vertrauen gegen gewählte CA",
          vpki
              ? "Kette zu einer unabhängig veröffentlichten BSI-Wurzel im mitgelieferten"
                    + " V-PKI-Truststore bestätigt."
              : "Kette gegen eine zusätzlich geladene CA bestätigt. Eine V-PKI-Zugehörigkeit ist"
                    + " damit nicht bestätigt.",
          path,
          "https://www.bsi.bund.de/VPKI");
      List<X509Certificate> resolved = new ArrayList<>();
      for (var c : result.getCertPath().getCertificates()) resolved.add((X509Certificate) c);
      resolved.add(root);
      JsonArray certificates = new JsonArray();
      for (X509Certificate certificate : resolved) {
        JsonObject item = new JsonObject();
        item.addProperty("subject", certificate.getSubjectX500Principal().getName());
        item.addProperty("sha256", VpkiTrustStore.fingerprint(certificate));
        item.addProperty("x5cIndex", chain.indexOf(certificate));
        certificates.add(item);
      }
      evidence.add("chain", certificates);
      return resolved;
    } catch (CertPathBuilderException untrusted) {
      evidence.addProperty("trusted", false);
      report.check(
          false,
          pem.isBlank() ? "V-PKI-Vertrauenskette" : "PKIX-Vertrauen gegen gewählte CA",
          "Keine gültige Kette zu einer mitgelieferten V-PKI-Wurzel oder zusätzlich gewählten CA."
              + " Zertifikate aus x5c allein erzeugen kein Vertrauen.",
          path,
          "https://www.bsi.bund.de/VPKI");
      return chain;
    }
  }
}
