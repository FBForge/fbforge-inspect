package de.fbforge.inspect;

import com.google.gson.JsonObject;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.cert.*;
import java.util.*;

/** Direct inspection of public PEM, DER and PKCS#7 certificates, without synthesizing a JWK. */
final class PublicCertificates {
  static void inspect(JsonObject input, Report report, PublicHttp http) throws Exception {
    String text = JsonFields.value(input, "text");
    if (text.contains("PRIVATE KEY"))
      throw new IllegalArgumentException("Nur öffentliche Zertifikate zulässig.");
    byte[] bytes;
    try {
      bytes =
          text.contains("-----BEGIN")
              ? text.getBytes(StandardCharsets.US_ASCII)
              : Base64.getMimeDecoder().decode(text);
      var parsed =
          CertificateFactory.getInstance("X.509")
              .generateCertificates(new ByteArrayInputStream(bytes));
      if (parsed.isEmpty() || parsed.size() > 10) throw new CertificateException();
      List<X509Certificate> chain = new ArrayList<>();
      for (var certificate : parsed) chain.add((X509Certificate) certificate);
      // Collection parsers need not preserve PKCS#7 ordering. Resolve an unambiguous chain.
      Set<X509Certificate> issuers = new HashSet<>();
      for (var child : chain)
        for (var parent : chain)
          if (child != parent
              && child.getIssuerX500Principal().equals(parent.getSubjectX500Principal()))
            issuers.add(parent);
      List<X509Certificate> leaves = chain.stream().filter(c -> !issuers.contains(c)).toList();
      if (leaves.size() != 1) throw new CertificateException();
      List<X509Certificate> ordered = new ArrayList<>();
      ordered.add(leaves.getFirst());
      while (ordered.size() < chain.size()) {
        X509Certificate child = ordered.getLast();
        List<X509Certificate> parents =
            chain.stream()
                .filter(
                    c ->
                        !ordered.contains(c)
                            && child.getIssuerX500Principal().equals(c.getSubjectX500Principal()))
                .toList();
        if (parents.size() != 1) throw new CertificateException();
        ordered.add(parents.getFirst());
      }
      Certificates.inspectChain(ordered, report, http, input, "$.certificate", "");
      report.add(
          "notApplicable",
          "Öffentliche Zertifikatsdatei",
          "Kein JWK eingegeben. Schlüsselbindung und aktive Verwendung an einem Zustellpunkt wurden"
              + " nicht geprüft.",
          "$",
          "",
          "",
          "Keine vollständige V-PKI-Profilkonformitätsprüfung.");
    } catch (CertificateException | IllegalArgumentException invalid) {
      throw new IllegalArgumentException(
          "Öffentliche PEM-, DER- oder PKCS#7-Zertifikatskette erwartet.");
    }
  }
}
