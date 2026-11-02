package de.fbforge.inspect;

import com.google.gson.*;
import java.io.*;
import java.security.*;
import java.security.cert.*;
import java.time.*;
import java.util.*;

/** Independently published V-PKI anchors; directory CAs are path material, never anchors. */
final class VpkiTrustStore {
  private static final String RESOURCE = "/de/fbforge/inspect/vpki/";
  private static List<Entry> cached;
  private static String asOf;

  record Entry(String name, String fingerprint, boolean root, X509Certificate certificate) {}

  private VpkiTrustStore() {}

  static synchronized List<Entry> entries() throws IOException, GeneralSecurityException {
    if (cached != null) return cached;
    JsonObject manifest;
    try (InputStream input = resource("manifest.json")) {
      manifest =
          JsonParser.parseReader(
                  new InputStreamReader(input, java.nio.charset.StandardCharsets.UTF_8))
              .getAsJsonObject();
    }
    if (manifest.get("schemaVersion").getAsInt() != 1)
      throw new IOException("V-PKI-Truststore hat eine unbekannte Version.");
    CertificateFactory factory = CertificateFactory.getInstance("X.509");
    List<Entry> entries = new ArrayList<>();
    Set<String> fingerprints = new HashSet<>();
    for (JsonElement value : manifest.getAsJsonArray("certificates")) {
      JsonObject item = value.getAsJsonObject();
      String filename = item.get("file").getAsString();
      if (!filename.matches("[a-z0-9.-]+\\.der"))
        throw new IOException("Ungültiger Truststore-Dateiname.");
      byte[] bytes;
      try (InputStream input = resource(filename)) {
        bytes = input.readNBytes(65537);
      }
      String fingerprint =
          HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
      if (bytes.length > 65536
          || !fingerprint.equals(item.get("sha256").getAsString())
          || !fingerprints.add(fingerprint))
        throw new IOException("V-PKI-Zertifikat stimmt nicht mit dem Inventar überein.");
      X509Certificate certificate =
          (X509Certificate) factory.generateCertificate(new ByteArrayInputStream(bytes));
      if (certificate.getBasicConstraints() < 0)
        throw new CertificateException("Kein CA-Zertifikat im Truststore.");
      boolean root = item.get("root").getAsBoolean();
      if (root) {
        if (!certificate.getSubjectX500Principal().equals(certificate.getIssuerX500Principal()))
          throw new CertificateException("V-PKI-Wurzel ist nicht selbst ausgestellt.");
        certificate.verify(certificate.getPublicKey());
      }
      entries.add(new Entry(item.get("name").getAsString(), fingerprint, root, certificate));
    }
    if (entries.isEmpty() || entries.size() > 128 || entries.stream().noneMatch(Entry::root))
      throw new IOException("V-PKI-Truststore ist leer oder überschreitet das Limit.");
    asOf = manifest.get("asOf").getAsString();
    cached = List.copyOf(entries);
    return cached;
  }

  static List<Entry> validEntries(Instant now) throws IOException, GeneralSecurityException {
    List<Entry> valid = new ArrayList<>();
    for (Entry entry : entries()) {
      try {
        entry.certificate().checkValidity(Date.from(now));
        valid.add(entry);
      } catch (CertificateExpiredException | CertificateNotYetValidException ignored) {
      }
    }
    return List.copyOf(valid);
  }

  static synchronized String asOf() throws IOException, GeneralSecurityException {
    entries();
    return asOf;
  }

  static String fingerprint(X509Certificate certificate) throws GeneralSecurityException {
    return HexFormat.of()
        .formatHex(MessageDigest.getInstance("SHA-256").digest(certificate.getEncoded()));
  }

  private static InputStream resource(String name) throws IOException {
    InputStream input = VpkiTrustStore.class.getResourceAsStream(RESOURCE + name);
    if (input == null) throw new IOException("Mitgelieferter V-PKI-Truststore fehlt.");
    return input;
  }
}
