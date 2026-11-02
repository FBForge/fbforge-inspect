package de.fbforge.inspect;

import com.google.gson.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.security.cert.*;
import java.time.Instant;
import java.util.*;
import org.bouncycastle.asn1.x509.*;
import org.bouncycastle.cert.jcajce.JcaX509CertificateHolder;
import org.bouncycastle.cert.ocsp.*;
import org.bouncycastle.operator.jcajce.JcaDigestCalculatorProviderBuilder;

/** Builds real OCSP POST probes and CRL GET probes from selected public certificates. */
final class CertificateEndpoints {
  private CertificateEndpoints() {}

  static List<NetworkDiagnostics.Target> probes(JsonObject input) throws Exception {
    JsonArray values = JsonFields.array(input, "certificates");
    if (values.size() > 12)
      throw new IllegalArgumentException(
          "Höchstens zwölf öffentliche Zertifikate pro Netzwerkprüfung.");
    if (values.isEmpty()) return List.of();
    List<X509Certificate> certificates = new ArrayList<>();
    CertificateFactory factory = CertificateFactory.getInstance("X.509");
    for (JsonElement value : values) {
      if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString())
        throw new IllegalArgumentException("Öffentliche Zertifikate als PEM erwartet.");
      String pem = value.getAsString();
      if (pem.length() > 65536 || pem.contains("PRIVATE KEY"))
        throw new IllegalArgumentException("Nur öffentliche Zertifikate sind zulässig.");
      certificates.add(
          (X509Certificate)
              factory.generateCertificate(
                  new ByteArrayInputStream(pem.getBytes(StandardCharsets.US_ASCII))));
    }
    List<X509Certificate> issuers = new ArrayList<>(certificates);
    for (var entry : VpkiTrustStore.validEntries(Instant.now())) issuers.add(entry.certificate());
    LinkedHashMap<String, NetworkDiagnostics.Target> probes = new LinkedHashMap<>();
    for (X509Certificate certificate : certificates) {
      if (certificate.getIssuerX500Principal().equals(certificate.getSubjectX500Principal()))
        continue;
      var holder = new JcaX509CertificateHolder(certificate);
      X509Certificate issuer = null;
      for (X509Certificate candidate : issuers) {
        if (!certificate.getIssuerX500Principal().equals(candidate.getSubjectX500Principal()))
          continue;
        try {
          certificate.verify(candidate.getPublicKey());
          issuer = candidate;
          break;
        } catch (java.security.GeneralSecurityException ignored) {
        }
      }
      var aia = AuthorityInformationAccess.fromExtensions(holder.getExtensions());
      if (issuer != null && aia != null) {
        CertificateID id =
            new CertificateID(
                new JcaDigestCalculatorProviderBuilder().build().get(CertificateID.HASH_SHA1),
                new JcaX509CertificateHolder(issuer),
                certificate.getSerialNumber());
        byte[] request = new OCSPReqBuilder().addRequest(id).build().getEncoded();
        for (AccessDescription access : aia.getAccessDescriptions()) {
          if (!access.getAccessMethod().equals(AccessDescription.id_ad_ocsp)
              || access.getAccessLocation().getTagNo() != GeneralName.uniformResourceIdentifier)
            continue;
          String url = access.getAccessLocation().getName().toString();
          try {
            PublicHttp.validateUri(URI.create(url), true);
          } catch (IllegalArgumentException unsupported) {
            continue;
          }
          probes.putIfAbsent(
              "POST " + url, new NetworkDiagnostics.Target("OCSP-Endpunkt", url, request, true));
        }
      }
      for (String url : CrlEndpoints.locations(CRLDistPoint.fromExtensions(holder.getExtensions())))
        probes.putIfAbsent(
            "GET " + url, new NetworkDiagnostics.Target("CRL-Endpunkt", url, null, true));
    }
    return List.copyOf(probes.values());
  }
}
