package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import org.bouncycastle.cert.ocsp.OCSPReq;
import org.junit.jupiter.api.*;

class NetworkDiagnosticsTest {
  @BeforeAll
  static void fixtures() throws Exception {
    CryptoTest.fixtures();
  }

  private static PublicHttp transport(Report report, List<String> postUrls) {
    return new PublicHttp(
        report,
        host -> new InetAddress[] {InetAddress.getByAddress(new byte[] {8, 8, 8, 8})},
        (uri, ip, post, budget) -> {
          if (post != null) {
            postUrls.add(uri.toString());
            try {
              assertEquals(CryptoTest.id, new OCSPReq(post).getRequestList()[0].getCertID());
            } catch (Exception failure) {
              throw new java.io.IOException(failure);
            }
          }
          return new PublicHttp.Response(
              200,
              Map.of("content-type", "application/json"),
              "{\"public\":true}".getBytes(StandardCharsets.UTF_8),
              uri);
        },
        System::nanoTime);
  }

  @Test
  void probesTheSelectedEnvironmentAndExportsConnectionEvidenceOnly() throws Exception {
    for (String environment : List.of("TEST", "STAGE", "PROD")) {
      Report report = new Report("network", environment, "v3");
      NetworkDiagnostics.inspect(new JsonObject(), report, transport(report, new ArrayList<>()));
      assertEquals(4, report.data.getAsJsonArray("networkChecks").size());
      assertEquals(4, report.requests.size());
      assertTrue(report.findings.stream().allMatch(f -> f.status().equals("confirmed")));
      assertTrue(report.rawResponses.isEmpty());
      assertTrue(
          report.requests.stream()
              .anyMatch(
                  r ->
                      r.url()
                          .startsWith(
                              "https://"
                                  + URI.create(Api.submission(environment)).getHost()
                                  + "/")));
      assertTrue(
          report.requests.stream()
              .anyMatch(
                  r ->
                      r.url()
                          .contains(
                              environment.equals("TEST")
                                  ? "stage.pvog.fitko.net"
                                  : "pvog.fitko.net")));
    }
  }

  @Test
  void certificateEndpointsUseARealOcspPostAndCrlGetWithTheirOwnPurpose() throws Exception {
    JsonObject input = new JsonObject();
    JsonArray certificates = new JsonArray();
    certificates.add(
        CryptoTest.pem(RevocationWorkflowTest.leaf(true, "https://example.org/current.crl")));
    certificates.add(CryptoTest.pem(CryptoTest.issuer));
    input.add("certificates", certificates);
    List<String> postUrls = new ArrayList<>();
    Report report = new Report("network", "PROD", "v3");
    NetworkDiagnostics.inspect(input, report, transport(report, postUrls));
    assertEquals(List.of("https://example.org/ocsp"), postUrls);
    assertEquals(6, report.data.getAsJsonArray("networkChecks").size());
    assertEquals(1, report.requests.stream().filter(r -> r.method().equals("POST")).count());
    assertTrue(report.rawResponses.isEmpty());
    assertTrue(
        report.findings.stream()
            .filter(f -> f.source().contains("example.org"))
            .allMatch(f -> f.limitation().contains("Sperrstatus")));
  }

  @Test
  void proxyFailuresRemainSeparateFromServiceAndCertificateFindings() throws Exception {
    Report report = new Report("network", "PROD", "v3");
    PublicHttp http =
        new PublicHttp(
            report,
            host -> new InetAddress[] {InetAddress.getByAddress(new byte[] {8, 8, 8, 8})},
            (uri, ip, post, budget) -> {
              throw new NetworkFailure("PROXY_AUTH_FAILED");
            },
            System::nanoTime);
    NetworkDiagnostics.inspect(new JsonObject(), report, http);
    assertEquals(4, report.findings.size());
    assertTrue(
        report.findings.stream()
            .allMatch(f -> f.status().equals("unknown") && f.detail().contains("Proxy-Anmeldung")));
    for (JsonElement row : report.data.getAsJsonArray("networkChecks"))
      assertEquals("PROXY_AUTH_FAILED", row.getAsJsonObject().get("code").getAsString());
    assertTrue(report.requests.stream().allMatch(r -> r.outcome().equals("PROXY_AUTH_FAILED")));
  }

  @Test
  void invalidCertificatesAndOversizedEndpointListsDoNotIssueRequests() throws Exception {
    Report report = new Report("network", "PROD", "v3");
    PublicHttp http = transport(report, new ArrayList<>());
    JsonObject input = new JsonObject();
    JsonArray certificates = new JsonArray();
    certificates.add("-----BEGIN PRIVATE KEY-----");
    input.add("certificates", certificates);
    assertThrows(
        IllegalArgumentException.class, () -> NetworkDiagnostics.inspect(input, report, http));
    assertTrue(report.requests.isEmpty());
    input.remove("certificates");
    JsonArray endpoints = new JsonArray();
    for (int i = 0; i < 13; i++) endpoints.add("https://example.org/" + i);
    input.add("certificateEndpoints", endpoints);
    assertThrows(
        IllegalArgumentException.class, () -> NetworkDiagnostics.inspect(input, report, http));
    assertTrue(report.requests.isEmpty());
  }

  @Test
  void serviceHttpErrorsAreVisibleAsResponsesRatherThanDnsOrTlsFailures() throws Exception {
    Report report = new Report("network", "PROD", "v3");
    PublicHttp http =
        new PublicHttp(
            report,
            host -> new InetAddress[] {InetAddress.getByAddress(new byte[] {8, 8, 8, 8})},
            (uri, ip, post, budget) -> new PublicHttp.Response(403, Map.of(), new byte[0], uri),
            System::nanoTime);
    NetworkDiagnostics.inspect(new JsonObject(), report, http);
    assertTrue(
        report.findings.stream()
            .allMatch(f -> f.status().equals("warning") && f.detail().startsWith("HTTP 403")));
    assertTrue(
        report.requests.stream().allMatch(r -> r.status() == 403 && r.outcome().equals("HTTP")));
  }
}
