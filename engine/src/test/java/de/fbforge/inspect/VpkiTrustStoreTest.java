package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.security.cert.*;
import java.time.*;
import java.util.*;
import org.junit.jupiter.api.Test;

class VpkiTrustStoreTest {
  private static final Instant SNAPSHOT = Instant.parse("2026-11-02T12:00:00Z");

  @Test
  void independentlyPinnedBsiRootsAndVerifiedDirectoryCasAreBundled() throws Exception {
    var entries = VpkiTrustStore.entries();
    assertEquals(36, entries.size());
    assertEquals("2026-11-02", VpkiTrustStore.asOf());
    Set<String> expected =
        Set.of(
            "6f17787cede05086161cb4cdf919d4a7b9510370de0283eada52fc5073a65b69",
            "a2668de261e4823ba6771c0c275451016a64f85ce15fe227f4eb5e8bef59ace7",
            "1d328479e165fd36fbc27e1d531168d13d60d66fa19b1b818036c2682d18ddd4",
            "7bc7b6af2664d6d284c7e9815c89d4268773b32692de422b2c09f32e33ccbdc2");
    assertEquals(
        expected,
        entries.stream()
            .filter(VpkiTrustStore.Entry::root)
            .map(VpkiTrustStore.Entry::fingerprint)
            .collect(java.util.stream.Collectors.toSet()));
    assertEquals(32, entries.stream().filter(e -> !e.root()).count());
    assertTrue(entries.stream().anyMatch(e -> e.name().equals("DOI CA 13") && !e.root()));
    assertFalse(entries.stream().anyMatch(e -> e.name().equals("Auswaertiges Amt Root CA")));
    for (var entry : entries) {
      assertEquals(entry.fingerprint(), VpkiTrustStore.fingerprint(entry.certificate()));
      entry.certificate().checkValidity(Date.from(SNAPSHOT));
    }
  }

  @Test
  void everyIntermediateBuildsToAPinnedRootWithoutPromotingTheIntermediate() throws Exception {
    for (var entry : VpkiTrustStore.entries()) {
      if (entry.root()) continue;
      Report report = new Report("keys", "PROD", "v3");
      var chain =
          CertificateTrust.inspect(
              List.of(entry.certificate()), new JsonObject(), report, "$.keys[0]", SNAPSHOT);
      assertTrue(
          report.findings.stream()
              .anyMatch(
                  f -> f.title().equals("V-PKI-Vertrauenskette") && f.status().equals("confirmed")),
          entry.name());
      var evidence = report.data.getAsJsonArray("certificateTrust").get(0).getAsJsonObject();
      assertEquals("bundled-vpki", evidence.get("source").getAsString());
      assertEquals(4, evidence.get("bundledRoots").getAsInt());
      assertEquals(0, evidence.get("additionalRoots").getAsInt());
      assertEquals(entry.certificate(), chain.getFirst());
      assertTrue(
          VpkiTrustStore.entries().stream()
              .anyMatch(root -> root.root() && root.certificate().equals(chain.getLast())));
    }
  }

  @Test
  void expiredAndNotYetValidCertificatesAreNotTrustMaterial() throws Exception {
    assertTrue(VpkiTrustStore.validEntries(Instant.parse("2010-01-01T00:00:00Z")).isEmpty());
    var future = VpkiTrustStore.validEntries(Instant.parse("2028-01-01T00:00:00Z"));
    assertFalse(future.stream().anyMatch(e -> e.name().equals("PCA-1-Verwaltung-17")));
    assertTrue(future.stream().anyMatch(e -> e.name().equals("PCA-1-Verwaltung-26") && e.root()));
    assertTrue(VpkiTrustStore.validEntries(Instant.parse("2050-01-01T00:00:00Z")).isEmpty());
  }

  @Test
  void expiredBundleDoesNotTrustTheReceivedRoot() throws Exception {
    var ca =
        VpkiTrustStore.entries().stream()
            .filter(e -> e.name().equals("DOI CA 13"))
            .findFirst()
            .orElseThrow();
    Report report = new Report("keys", "PROD", "v3");
    CertificateTrust.inspect(
        List.of(ca.certificate()),
        new JsonObject(),
        report,
        "$.keys[0]",
        Instant.parse("2050-01-01T00:00:00Z"));
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f -> f.code() == FindingCode.TRUST_ANCHOR_MISSING && f.status().equals("unknown")));
    assertFalse(report.findings.stream().anyMatch(f -> f.status().equals("confirmed")));
  }
}
