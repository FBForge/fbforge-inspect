package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.cert.*;
import java.util.*;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.io.TempDir;

class PortableWorkflowTest {
  @TempDir Path directory;
  static final String ID = SignatureFixtures.DESTINATION;

  static JsonObject json(String text) {
    return JsonParser.parseString(text).getAsJsonObject();
  }

  static JsonObject job() {
    return json(
        """
        {"format":"fbforge-inspect-job","formatVersion":1,"environment":"TEST","apiVersion":"v3",
         "checks":[{"id":"local","operation":"schema","input":{"text":"{\\\"type\\\":\\\"object\\\"}"}}]}
        """);
  }

  static PublicHttp fake(Report report, java.util.function.Function<String, String> body) {
    return new PublicHttp(
        report,
        host -> new InetAddress[] {InetAddress.getByAddress(new byte[] {8, 8, 8, 8})},
        (uri, ip, post, budget) ->
            SignatureFixtures.response(uri.toString(), body.apply(uri.toString())),
        System::nanoTime);
  }

  @Test
  void portableJobRunsThroughTheSameEngineAndRemovesRawBodies() throws Exception {
    JsonObject job = job();
    JsonObject result = Cli.run(job, PortableJob.requests(job), new JsonObject(), "a".repeat(64));
    assertEquals(0, result.get("exitCode").getAsInt());
    JsonObject report =
        result.getAsJsonArray("checks").get(0).getAsJsonObject().getAsJsonObject("report");
    assertEquals("schema", report.get("operation").getAsString());
    assertFalse(report.getAsJsonObject("data").getAsJsonArray("structure").isEmpty());
    assertFalse(report.has("rawResponses"));
    assertFalse(result.getAsJsonObject("runtime").has("hostname"));
  }

  @Test
  void publicJobRejectsSecretsDuplicatesAndUnboundedWork() {
    for (String field : List.of("username", "password", "authorization", "token")) {
      JsonObject job = job();
      JsonObject network = new JsonObject();
      network.addProperty("mode", "direct");
      network.addProperty(field, "synthetic-not-a-secret");
      job.add("network", network);
      assertThrows(IllegalArgumentException.class, () -> PortableJob.requests(job));
    }
    JsonObject duplicate = job();
    duplicate.getAsJsonArray("checks").add(duplicate.getAsJsonArray("checks").get(0).deepCopy());
    assertThrows(IllegalArgumentException.class, () -> PortableJob.requests(duplicate));
    JsonObject empty = job();
    empty.add("checks", new JsonArray());
    assertThrows(IllegalArgumentException.class, () -> PortableJob.requests(empty));
    JsonObject privateKey = job();
    privateKey.add("private", json("{\"kty\":\"RSA\",\"d\":\"synthetic\"}"));
    assertThrows(IllegalArgumentException.class, () -> PortableJob.requests(privateKey));
  }

  @Test
  void invalidCheckIsAnExplicitPartialResult() throws Exception {
    JsonObject job = job();
    job.getAsJsonArray("checks")
        .get(0)
        .getAsJsonObject()
        .add("input", json("{\"text\":\"invalid\"}"));
    JsonObject result = Cli.run(job, PortableJob.requests(job), new JsonObject(), "a".repeat(64));
    assertEquals(3, result.get("exitCode").getAsInt());
    assertEquals(
        "CHECK_FAILED",
        result.getAsJsonArray("checks").get(0).getAsJsonObject().get("error").getAsString());
  }

  @Test
  void cliWritesUtf8AndNeverOverwritesAnExistingResult() throws Exception {
    Path input = directory.resolve("job.json"), output = directory.resolve("result.json");
    Files.writeString(input, job().toString());
    var stdout = new ByteArrayOutputStream();
    var stderr = new ByteArrayOutputStream();
    String[] args = {"--job", input.toString(), "--output", output.toString()};
    assertEquals(0, Cli.execute(args, new PrintStream(stdout), new PrintStream(stderr)));
    assertEquals(
        "fbforge-inspect-result", json(Files.readString(output)).get("format").getAsString());
    byte[] original = Files.readAllBytes(output);
    assertEquals(74, Cli.execute(args, new PrintStream(stdout), new PrintStream(stderr)));
    assertArrayEquals(original, Files.readAllBytes(output));
    assertFalse(stderr.toString(StandardCharsets.UTF_8).contains(directory.toString()));
  }

  @Test
  void cliRejectsUnknownOptionsAndProvidesHelpWithoutFiles() {
    var out = new PrintStream(new ByteArrayOutputStream());
    assertEquals(0, Cli.execute(new String[] {"--help"}, out, out));
    assertEquals(0, Cli.execute(new String[] {"--version"}, out, out));
    for (String[] args :
        List.of(
            new String[] {},
            new String[] {"--job"},
            new String[] {"--secret", "value"},
            new String[] {"--job", "a", "--output", "b", "--job", "c"}))
      assertEquals(64, Cli.execute(args, out, out));
  }

  @Test
  void oversizedInputIsBoundedBeforeParsing() throws Exception {
    Path input = directory.resolve("large");
    Files.write(input, new byte[1025]);
    assertThrows(IOException.class, () -> Cli.read(input, 1024));
  }

  @Test
  void privateJwkTextAndExcessiveStructuredNestingAreRejectedAtThePortableBoundary() {
    JsonObject privateJob = job();
    JsonObject check = privateJob.getAsJsonArray("checks").get(0).getAsJsonObject();
    check.addProperty("operation", "keys");
    check
        .getAsJsonObject("input")
        .addProperty("text", "{\"kty\":\"RSA\",\"d\":\"synthetic-private-marker\"}");
    assertThrows(IllegalArgumentException.class, () -> PortableJob.requests(privateJob));
    JsonObject root = new JsonObject(), cursor = root;
    for (int index = 0; index < 100; index++) {
      JsonObject child = new JsonObject();
      cursor.add("child", child);
      cursor = child;
    }
    assertThrows(IllegalArgumentException.class, () -> JsonBounds.check(root));
    assertDoesNotThrow(() -> JsonBounds.check(job()));
  }

  @Test
  void reverseRoutingDistinguishesMissingAndIncompleteResults() throws Exception {
    for (boolean complete : List.of(true, false)) {
      Report report = new Report("reverse", "TEST", "v3");
      PublicHttp http =
          fake(
              report,
              url ->
                  url.contains("/routes?")
                      ? "{\"routes\":[],\"offset\":0,\"count\":0,\"totalCount\":"
                          + (complete ? "0" : "1")
                          + "}"
                      : "{\"destinationId\":\""
                          + ID
                          + "\",\"status\":\"active\",\"type\":\"A\",\"metadataVersions\":[\"2.0.0\"],\"publicServices\":[{\"identifier\":\"99008001014002\",\"regions\":[\"DE02\"]}]}");
      JsonObject input =
          json(
              "{\"destinationId\":\""
                  + ID
                  + "\",\"pairs\":[{\"service\":\"99008001014002\",\"region\":\"020000000000\",\"regionKind\":\"ars\"}]}");
      ReverseRouting.inspect(new Api(report, http::get), http, input);
      var row = report.data.getAsJsonArray("reverseRoutes").get(0).getAsJsonObject();
      assertEquals(complete, row.get("complete").getAsBoolean());
      assertTrue(row.get("publishedContext").getAsBoolean());
      assertFalse(row.get("expectedFound").getAsBoolean());
      assertEquals(complete ? "warning" : "unknown", report.findings.getLast().status());
    }
  }

  @Test
  void reversePreparationDoesNotLaunchRoutingAndStageNeverFallsBack() throws Exception {
    Report report = new Report("reverse", "TEST", "v3");
    AtomicInteger routes = new AtomicInteger();
    PublicHttp http =
        fake(
            report,
            url -> {
              if (url.contains("routes")) routes.incrementAndGet();
              return "{}";
            });
    ReverseRouting.inspect(
        new Api(report, http::get), http, json("{\"destinationId\":\"" + ID + "\"}"));
    assertEquals(0, routes.get());
    Report stage = new Report("reverse", "STAGE", "v3");
    assertThrows(
        IllegalArgumentException.class,
        () -> ReverseRouting.inspect(new Api(stage, http::get), http, new JsonObject()));
  }

  @Test
  void directCertificateInspectionDoesNotInventAJwkBinding() throws Exception {
    CryptoTest.fixtures();
    Report report = new Report("certificate", "TEST", "v3");
    JsonObject input = new JsonObject();
    input.addProperty("text", CryptoTest.pem(CryptoTest.leaf) + CryptoTest.pem(CryptoTest.issuer));
    input.addProperty("revocation", false);
    PublicCertificates.inspect(input, report, new PublicHttp(report));
    assertEquals(2, report.data.getAsJsonArray("certificates").size());
    assertTrue(
        report.findings.stream().noneMatch(f -> f.code() == FindingCode.CERTIFICATE_BINDING));
    input.addProperty("text", Base64.getEncoder().encodeToString(CryptoTest.leaf.getEncoded()));
    Report der = new Report("certificate", "TEST", "v3");
    PublicCertificates.inspect(input, der, new PublicHttp(der));
    assertEquals(1, der.data.getAsJsonArray("certificates").size());
  }

  @Test
  void httpsTrustOnlyAcceptsExplicitPublicCaCertificates() throws Exception {
    CryptoTest.fixtures();
    assertNotNull(HttpsTrust.factory(CryptoTest.pem(CryptoTest.issuer)));
    assertThrows(
        IllegalArgumentException.class, () -> HttpsTrust.factory(CryptoTest.pem(CryptoTest.leaf)));
    assertThrows(
        IllegalArgumentException.class, () -> HttpsTrust.factory("-----BEGIN PRIVATE KEY-----"));
    assertThrows(IllegalArgumentException.class, () -> HttpsTrust.factory("invalid"));
    JsonObject direct =
        json("{\"mode\":\"direct\",\"username\":\"ignored\",\"password\":\"ignored\"}");
    direct.addProperty("httpsCaPem", CryptoTest.pem(CryptoTest.issuer));
    NetworkOptions options = NetworkOptions.read(direct);
    assertFalse(options.manual());
    assertNull(options.authorization());
    assertFalse(options.httpsCaPem.isBlank());
  }

  @Test
  void tlsClassificationUsesFixedCodesAndPreservesUnknownCauses() {
    assertEquals("TLS_EXPIRED", NetworkFailure.tlsCode(new CertificateExpiredException()));
    assertEquals(
        "TLS_NOT_YET_VALID", NetworkFailure.tlsCode(new CertificateNotYetValidException()));
    assertEquals("TLS_TRUST_UNKNOWN", NetworkFailure.tlsCode(new CertPathBuilderException()));
    assertEquals(
        "TLS_HOSTNAME",
        NetworkFailure.tlsCode(new CertificateException("No name matching fixture")));
    assertEquals(
        "TLS_CERTIFICATE_INVALID",
        NetworkFailure.tlsCode(new CertificateException("synthetic secret not returned")));
    assertEquals("TLS_FAILED", NetworkFailure.tlsCode(new IOException()));
  }
}
