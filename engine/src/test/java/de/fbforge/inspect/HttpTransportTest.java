package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.*;
import java.util.concurrent.atomic.*;
import javax.net.ssl.*;
import org.junit.jupiter.api.*;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class HttpTransportTest {
  private static final URI SOURCE = URI.create("https://example.org/data");

  private static InetAddress ip(String value) throws IOException {
    return InetAddress.getByName(value);
  }

  private static NetworkBudget budget() {
    return new NetworkBudget(System::nanoTime);
  }

  private static PublicHttp.Response read(String response, NetworkBudget budget)
      throws IOException {
    return HttpResponseReader.read(
        new ByteArrayInputStream(response.getBytes(StandardCharsets.ISO_8859_1)), SOURCE, budget);
  }

  private static PublicHttp.Response ok(URI uri) {
    return new PublicHttp.Response(200, Map.of(), "{}".getBytes(StandardCharsets.UTF_8), uri);
  }

  private static Report report() {
    return new Report("destination", "TEST", "v3");
  }

  @Test
  void readsLengthChunkedAndCloseDelimitedResponses() throws Exception {
    assertEquals("abc", read("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc", budget()).text());
    assertEquals(
        "abcd",
        read(
                "HTTP/1.1 200 OK\r\n"
                    + "Transfer-Encoding: CHUNKED\r\n\r\n"
                    + "2;extension=1\r\n"
                    + "ab\r\n"
                    + "2\r\n"
                    + "cd\r\n"
                    + "0\r\n"
                    + "X-Proof: yes\r\n\r\n",
                budget())
            .text());
    assertEquals("body", read("HTTP/1.0 404 Missing\r\n\r\nbody", budget()).text());
    assertFalse(read("HTTP/1.1 404 Missing\r\nContent-Length: 0\r\n\r\n", budget()).ok());
  }

  @ParameterizedTest
  @ValueSource(
      strings = {
        "HTTP/2 200 OK\r\n\r\n",
        "HTTP/1.1 nope\r\n\r\n",
        "HTTP/1.1 200 OK\n\n",
        "HTTP/1.1 200 OK\r\nBad Header: a\r\n\r\n",
        "HTTP/1.1 200 OK\r\nmissing-colon\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n0\r\n\r\n",
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: -1\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 999999999999999\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 4194305\r\n\r\n",
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n-1\r\n",
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nffffffffff\r\n",
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\naX\r\n",
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nContent-Length: 1\r\n\r\n"
      })
  void rejectsMalformedOrAmbiguousFraming(String input) {
    assertThrows(IOException.class, () -> read(input, budget()));
  }

  @Test
  void boundsHeadersLinesAndBothBodyFramings() {
    assertThrows(
        IOException.class,
        () -> read("HTTP/1.1 200 OK\r\nX:" + "a".repeat(8193) + "\r\n\r\n", budget()));
    assertThrows(
        IOException.class,
        () ->
            read(
                "HTTP/1.1 200 OK\r\n" + ("X: " + "a".repeat(4000) + "\r\n").repeat(18) + "\r\n",
                budget()));
    assertThrows(
        IOException.class,
        () -> read("HTTP/1.1 200 OK\r\n\r\n" + "a".repeat(PublicHttp.MAX_BODY + 1), budget()));
    assertThrows(
        IOException.class,
        () -> read("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n400001\r\n", budget()));
  }

  @Test
  void acceptsOpaqueHeaderTextButRejectsControlCharacters() throws Exception {
    assertEquals(
        "ä",
        read("HTTP/1.1 200 OK\r\nX-Label: ä\r\nContent-Length: 0\r\n\r\n", budget())
            .headers()
            .get("x-label"));
    assertThrows(
        IOException.class, () -> read("HTTP/1.1 200 OK\r\nX-Label: a\u0001b\r\n\r\n", budget()));
  }

  @Test
  void failedTransfersStillConsumeTheAggregateBudget() {
    NetworkBudget budget = budget();
    assertThrows(
        EOFException.class, () -> read("HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nabc", budget));
    assertEquals(NetworkBudget.MAX_TOTAL - 3, budget.remaining());
  }

  @Test
  void closeDelimitedReadsUseOnlyTheRemainingBudgetAndOneOverflowProbe() throws Exception {
    NetworkBudget budget = budget();
    budget.charge((int) NetworkBudget.MAX_TOTAL - 2);
    String wire = "HTTP/1.1 200 OK\r\n\r\n" + "x".repeat(100);
    assertThrows(IOException.class, () -> read(wire, budget));
    assertEquals(-1, budget.remaining());
  }

  @Test
  void exactAggregateBudgetAllowsMultipleChunksButNoFurtherBody() throws Exception {
    NetworkBudget budget = budget();
    budget.charge((int) NetworkBudget.MAX_TOTAL - 4);
    assertEquals(
        "abcd",
        read(
                "HTTP/1.1 200 OK\r\n"
                    + "Transfer-Encoding: chunked\r\n\r\n"
                    + "2\r\n"
                    + "ab\r\n"
                    + "2\r\n"
                    + "cd\r\n"
                    + "0\r\n\r\n",
                budget)
            .text());
    assertEquals(0, budget.remaining());
    assertThrows(
        IOException.class, () -> read("HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\na", budget));
  }

  @Test
  void checksDeadlineDuringHeaderReadingAndBeforeNewRequests() {
    AtomicLong clock = new AtomicLong();
    NetworkBudget budget =
        new NetworkBudget(() -> clock.getAndAdd(Duration.ofSeconds(30).toNanos()));
    assertThrows(IOException.class, () -> read("HTTP/1.1 200 OK\r\n\r\n", budget));
    AtomicLong stable = new AtomicLong();
    NetworkBudget remaining = new NetworkBudget(stable::get);
    assertEquals(8000, assertDoesNotThrow(() -> remaining.timeoutMillis(8000)));
    stable.set(Duration.ofSeconds(120).toNanos());
    assertThrows(IOException.class, remaining::startRequest);
  }

  @Test
  void validatesEveryResolvedAddressBeforeExchange() throws Exception {
    AtomicInteger calls = new AtomicInteger();
    PublicHttp http =
        new PublicHttp(
            report(),
            host -> new InetAddress[] {ip("8.8.8.8"), ip("127.0.0.1")},
            (uri, address, post, b) -> {
              calls.incrementAndGet();
              return ok(uri);
            },
            System::nanoTime);
    assertThrows(IOException.class, () -> http.get(SOURCE.toString()));
    assertEquals(0, calls.get());
    PublicHttp empty =
        new PublicHttp(
            report(), host -> new InetAddress[0], (u, a, p, b) -> ok(u), System::nanoTime);
    assertThrows(IOException.class, () -> empty.get(SOURCE.toString()));
  }

  @Test
  void successfulRequestsUseTheCheckedIpAndRetainOnlyPublicBodies() throws Exception {
    Report report = report();
    InetAddress expected = ip("8.8.8.8");
    PublicHttp http =
        new PublicHttp(
            report,
            host -> new InetAddress[] {expected},
            (uri, address, post, b) -> {
              assertSame(expected, address);
              return ok(uri);
            },
            System::nanoTime);
    assertTrue(http.get(SOURCE.toString()).ok());
    assertEquals(1, report.requests.size());
    assertEquals("{}", report.rawResponses.get(SOURCE.toString()));
    PublicHttp secret =
        new PublicHttp(
            report,
            host -> new InetAddress[] {expected},
            (uri, a, p, b) ->
                new PublicHttp.Response(
                    200,
                    Map.of(),
                    "{\"kty\":\"oct\",\"k\":\"dummy\"}".getBytes(StandardCharsets.UTF_8),
                    uri),
            System::nanoTime);
    secret.get("https://example.org/private");
    assertFalse(report.rawResponses.containsKey("https://example.org/private"));
  }

  @Test
  void redirectsAreRevalidatedAndDowngradesOrOcspRedirectsFail() throws Exception {
    AtomicInteger calls = new AtomicInteger();
    Report report = report();
    PublicHttp http =
        new PublicHttp(
            report,
            host -> new InetAddress[] {ip("8.8.8.8")},
            (uri, a, p, b) -> {
              calls.incrementAndGet();
              return new PublicHttp.Response(
                  302,
                  Map.of("location", "https://example.org/next?token=dummy"),
                  new byte[0],
                  uri);
            },
            System::nanoTime);
    assertThrows(IllegalArgumentException.class, () -> http.get(SOURCE.toString()));
    assertEquals(1, calls.get());
    PublicHttp downgrade =
        new PublicHttp(
            report,
            host -> new InetAddress[] {ip("8.8.8.8")},
            (uri, a, p, b) ->
                new PublicHttp.Response(
                    302, Map.of("location", "http://example.org/next"), new byte[0], uri),
            System::nanoTime);
    assertThrows(IOException.class, () -> downgrade.pki(SOURCE.toString(), null));
    assertThrows(IOException.class, () -> downgrade.pki(SOURCE.toString(), new byte[] {1}));
  }

  @Test
  void followsRelativeRedirectsButBoundsAttemptsAndDeadlineAfterDns() throws Exception {
    AtomicInteger calls = new AtomicInteger();
    PublicHttp http =
        new PublicHttp(
            report(),
            host -> new InetAddress[] {ip("8.8.8.8")},
            (uri, a, p, b) ->
                calls.incrementAndGet() == 1
                    ? new PublicHttp.Response(302, Map.of("location", "/next"), new byte[0], uri)
                    : ok(uri),
            System::nanoTime);
    assertEquals("/next", http.get(SOURCE.toString()).uri().getPath());
    for (int i = 0; i < 38; i++) http.get(SOURCE.toString());
    assertThrows(IOException.class, () -> http.get(SOURCE.toString()));
    assertEquals(40, calls.get());
    AtomicLong clock = new AtomicLong();
    PublicHttp slow =
        new PublicHttp(
            report(),
            host -> {
              clock.set(Duration.ofSeconds(121).toNanos());
              return new InetAddress[] {ip("8.8.8.8")};
            },
            (u, a, p, b) -> fail("exchange after deadline"),
            clock::get);
    assertThrows(IOException.class, () -> slow.get(SOURCE.toString()));
  }

  @Test
  void redirectLoopStopsAfterFiveResponsesAndErrorsAreNotRetained() throws Exception {
    Report report = report();
    AtomicInteger calls = new AtomicInteger();
    PublicHttp http =
        new PublicHttp(
            report,
            host -> new InetAddress[] {ip("8.8.8.8")},
            (uri, a, p, b) -> {
              calls.incrementAndGet();
              return new PublicHttp.Response(302, Map.of("location", "/again"), new byte[0], uri);
            },
            System::nanoTime);
    assertThrows(IOException.class, () -> http.get(SOURCE.toString()));
    assertEquals(5, calls.get());
    assertTrue(report.rawResponses.isEmpty());
    assertTrue(report.requests.stream().anyMatch(request -> request.status() == 0));
  }

  @Test
  void requestEncodingAndTlsIdentityKeepTheOriginalHost() throws Exception {
    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    PinnedHttp.writeRequest(
        bytes, URI.create("https://example.org/a%2Fb?q=a%20b"), new byte[] {1, 2, 3});
    String text = bytes.toString(StandardCharsets.ISO_8859_1);
    assertTrue(text.startsWith("POST /a%2Fb?q=a%20b HTTP/1.1\r\nHost: example.org\r\n"));
    assertTrue(text.contains("Content-Length: 3\r\n"));
    assertTrue(text.contains("application/ocsp-request"));
    bytes.reset();
    PinnedHttp.writeRequest(bytes, URI.create("https://example.org"), null);
    assertTrue(bytes.toString(StandardCharsets.US_ASCII).startsWith("GET / HTTP/1.1"));
    try (SSLSocket socket =
        (SSLSocket) ((SSLSocketFactory) SSLSocketFactory.getDefault()).createSocket()) {
      PinnedHttp.configureTls(socket, SOURCE);
      assertEquals("HTTPS", socket.getSSLParameters().getEndpointIdentificationAlgorithm());
      assertEquals(
          "example.org",
          ((SNIHostName) socket.getSSLParameters().getServerNames().getFirst()).getAsciiName());
    }
  }
}
