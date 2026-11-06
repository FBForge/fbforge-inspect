package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.security.cert.X509Certificate;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.*;
import javax.net.ssl.*;
import org.bouncycastle.asn1.x500.X500Name;
import org.bouncycastle.asn1.x509.*;
import org.bouncycastle.cert.jcajce.*;
import org.bouncycastle.operator.jcajce.JcaContentSignerBuilder;
import org.junit.jupiter.api.*;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class ProxyNetworkTest {
  private static final String USER = "fixture-user", PASSWORD = "fixture-password";
  private static SSLContext serverTls, clientTls;
  private static final URI TARGET = URI.create("https://proxy-target.example/data");

  @BeforeAll
  static void fixtures() throws Exception {
    CryptoTest.fixtures();
    var builder =
        new JcaX509v3CertificateBuilder(
            new X500Name("CN=Synthetic Test CA"),
            java.math.BigInteger.valueOf(73),
            Date.from(Instant.now().minusSeconds(60)),
            Date.from(Instant.now().plusSeconds(3600)),
            new X500Name("CN=proxy-target.example"),
            CryptoTest.leafKey.getPublic());
    builder.addExtension(
        org.bouncycastle.asn1.x509.Extension.subjectAlternativeName,
        false,
        new GeneralNames(
            new GeneralName[] {
              new GeneralName(GeneralName.dNSName, "proxy-target.example"),
              new GeneralName(GeneralName.iPAddress, "127.0.0.1")
            }));
    X509Certificate certificate =
        new JcaX509CertificateConverter()
            .getCertificate(
                builder.build(
                    new JcaContentSignerBuilder("SHA256withRSA")
                        .build(CryptoTest.issuerKey.getPrivate())));
    KeyStore keys = KeyStore.getInstance("PKCS12");
    keys.load(null, null);
    keys.setKeyEntry(
        "fixture",
        CryptoTest.leafKey.getPrivate(),
        "fixture".toCharArray(),
        new java.security.cert.Certificate[] {certificate, CryptoTest.issuer});
    KeyManagerFactory km = KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm());
    km.init(keys, "fixture".toCharArray());
    serverTls = SSLContext.getInstance("TLS");
    serverTls.init(km.getKeyManagers(), null, null);
    KeyStore trust = KeyStore.getInstance("PKCS12");
    trust.load(null, null);
    trust.setCertificateEntry("fixture-ca", CryptoTest.issuer);
    TrustManagerFactory tm =
        TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm());
    tm.init(trust);
    clientTls = SSLContext.getInstance("TLS");
    clientTls.init(null, tm.getTrustManagers(), null);
  }

  private static NetworkOptions options(int port, String scheme) {
    JsonObject json = new JsonObject();
    json.addProperty("mode", "manual");
    json.addProperty("proxyUrl", scheme + "://127.0.0.1:" + port);
    json.addProperty("username", USER);
    json.addProperty("password", PASSWORD);
    return NetworkOptions.read(json);
  }

  private static InetAddress publicIp() throws Exception {
    return InetAddress.getByAddress(new byte[] {8, 8, 8, 8});
  }

  private static String head(InputStream input) throws Exception {
    NetworkBudget budget = new NetworkBudget(System::nanoTime);
    StringBuilder result = new StringBuilder();
    String line;
    while (!(line = HttpResponseReader.line(input, budget)).isEmpty())
      result.append(line).append("\r\n");
    return result.toString();
  }

  private static void response(OutputStream output) throws Exception {
    output.write(
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".getBytes(StandardCharsets.US_ASCII));
    output.flush();
  }

  private static SSLSocket server(Socket socket) throws Exception {
    SSLSocket tls =
        (SSLSocket)
            serverTls.getSocketFactory().createSocket(socket, "127.0.0.1", socket.getPort(), true);
    tls.setUseClientMode(false);
    tls.setSoTimeout(5000);
    tls.startHandshake();
    return tls;
  }

  @Test
  void basicHttpProxyUsesThePinnedPublicIpAndNeverExportsCredentials() throws Exception {
    try (ServerSocket listener = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
      listener.setSoTimeout(5000);
      FutureTask<String> capture =
          new FutureTask<>(
              () -> {
                try (Socket socket = listener.accept()) {
                  socket.setSoTimeout(5000);
                  String headers = head(socket.getInputStream());
                  response(socket.getOutputStream());
                  return headers;
                }
              });
      Thread.ofVirtual().start(capture);
      Report report = new Report("network", "PROD", "v3");
      PublicHttp http =
          new PublicHttp(
              report,
              host -> new InetAddress[] {publicIpUnchecked()},
              options(listener.getLocalPort(), "http"),
              System::nanoTime);
      assertEquals("ok", http.pki("http://proxy-target.example/list.crl", null).text());
      String headers = capture.get(8, TimeUnit.SECONDS);
      assertTrue(headers.startsWith("GET http://8.8.8.8/list.crl HTTP/1.1"));
      assertTrue(headers.contains("Host: proxy-target.example\r\n"));
      assertTrue(
          headers.contains(
              "Proxy-Authorization: " + options(listener.getLocalPort(), "http").authorization()));
      String exported = Main.JSON.toJson(report);
      assertFalse(exported.contains(PASSWORD));
      assertFalse(exported.contains(USER));
      assertFalse(exported.contains(options(listener.getLocalPort(), "http").authorization()));
      assertTrue(report.requests.getFirst().viaProxy());
      assertNotNull(report.requests.getFirst().durationMs());
    }
  }

  private static InetAddress publicIpUnchecked() {
    try {
      return publicIp();
    } catch (Exception e) {
      throw new IllegalStateException(e);
    }
  }

  @ParameterizedTest
  @ValueSource(strings = {"http", "https"})
  void connectPreservesHostnameTlsAndDoesNotForwardProxyAuthenticationToTheTarget(String scheme)
      throws Exception {
    try (ServerSocket listener = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
      listener.setSoTimeout(5000);
      FutureTask<List<String>> capture =
          new FutureTask<>(
              () -> {
                try (Socket raw = listener.accept()) {
                  raw.setSoTimeout(5000);
                  Socket proxy = scheme.equals("https") ? server(raw) : raw;
                  String connect = head(proxy.getInputStream());
                  proxy
                      .getOutputStream()
                      .write(
                          "HTTP/1.1 200 Connection established\r\n\r\n"
                              .getBytes(StandardCharsets.US_ASCII));
                  proxy.getOutputStream().flush();
                  try (SSLSocket target = server(proxy)) {
                    String request = head(target.getInputStream());
                    response(target.getOutputStream());
                    return List.of(connect, request);
                  }
                }
              });
      Thread.ofVirtual().start(capture);
      PublicHttp.Response result =
          PinnedHttp.exchange(
              TARGET,
              publicIp(),
              null,
              new NetworkBudget(System::nanoTime),
              options(listener.getLocalPort(), scheme),
              clientTls.getSocketFactory());
      assertEquals("ok", result.text());
      List<String> captured = capture.get(8, TimeUnit.SECONDS);
      assertTrue(captured.get(0).startsWith("CONNECT 8.8.8.8:443 HTTP/1.1"));
      assertTrue(captured.get(0).contains("Proxy-Authorization: Basic "));
      assertTrue(captured.get(1).startsWith("GET /data HTTP/1.1"));
      assertTrue(captured.get(1).contains("Host: proxy-target.example\r\n"));
      assertFalse(captured.get(1).toLowerCase(Locale.ROOT).contains("proxy-authorization"));
      assertFalse(captured.get(1).contains(PASSWORD));
    }
  }

  @ParameterizedTest
  @ValueSource(strings = {"target-hostname", "custom-ca-target-hostname", "proxy-hostname", "proxy-untrusted"})
  void rejectsWrongTlsHostsAndUntrustedProxyCertificates(String scenario) throws Exception {
    try (ServerSocket listener = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
      listener.setSoTimeout(5000);
      FutureTask<Boolean> peer =
          new FutureTask<>(
              () -> {
                try (Socket raw = listener.accept()) {
                  raw.setSoTimeout(5000);
                  if (scenario.endsWith("target-hostname")) {
                    head(raw.getInputStream());
                    raw.getOutputStream()
                        .write(
                            "HTTP/1.1 200 Connected\r\n\r\n".getBytes(StandardCharsets.US_ASCII));
                    raw.getOutputStream().flush();
                  }
                  try (SSLSocket tls = server(raw)) {
                    head(tls.getInputStream());
                  }
                  return false;
                } catch (IOException expected) {
                  return true;
                }
              });
      Thread.ofVirtual().start(peer);
      NetworkOptions proxy =
          options(listener.getLocalPort(), scenario.endsWith("target-hostname") ? "http" : "https");
      if (scenario.equals("proxy-hostname")) {
        JsonObject json = new JsonObject();
        json.addProperty("mode", "manual");
        json.addProperty("proxyUrl", "https://localhost:" + listener.getLocalPort());
        proxy = NetworkOptions.read(json);
      }
      final NetworkOptions selected = proxy;
      SSLSocketFactory factory =
          scenario.equals("custom-ca-target-hostname") ? HttpsTrust.factory(CryptoTest.pem(CryptoTest.issuer)) : scenario.equals("proxy-untrusted")
              ? (SSLSocketFactory) SSLSocketFactory.getDefault()
              : clientTls.getSocketFactory();
      URI target =
          scenario.endsWith("target-hostname")
              ? URI.create("https://wrong-host.example/data")
              : TARGET;
      NetworkFailure failure =
          assertThrows(
              NetworkFailure.class,
              () ->
                  PinnedHttp.exchange(
                      target,
                      publicIp(),
                      null,
                      new NetworkBudget(System::nanoTime),
                      selected,
                      factory));
      assertEquals(
          scenario.endsWith("target-hostname")
              ? "TLS_HOSTNAME"
              : scenario.equals("proxy-hostname")
                  ? "PROXY_TLS_HOSTNAME"
                  : "PROXY_TLS_TRUST_UNKNOWN",
          failure.code());
      assertTrue(peer.get(8, TimeUnit.SECONDS));
    }
  }

  @ParameterizedTest
  @ValueSource(strings = {"http", "https"})
  void explicitHttpsCaSupportsBothTlsPeersAndRecordsPublicPhasesWithoutCredentials(String scheme)
      throws Exception {
    try (ServerSocket listener = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
      listener.setSoTimeout(5000);
      FutureTask<String> peer =
          new FutureTask<>(
              () -> {
                try (Socket raw = listener.accept()) {
                  raw.setSoTimeout(5000);
                  Socket proxy = scheme.equals("https") ? server(raw) : raw;
                  head(proxy.getInputStream());
                  proxy
                      .getOutputStream()
                      .write("HTTP/1.1 200 Connected\r\n\r\n".getBytes(StandardCharsets.US_ASCII));
                  proxy.getOutputStream().flush();
                  try (SSLSocket target = server(proxy)) {
                    String request = head(target.getInputStream());
                    response(target.getOutputStream());
                    return request;
                  }
                }
              });
      Thread.ofVirtual().start(peer);
      JsonObject settings = new JsonObject();
      settings.addProperty("mode", "manual");
      settings.addProperty("proxyUrl", scheme + "://127.0.0.1:" + listener.getLocalPort());
      settings.addProperty("username", USER);
      settings.addProperty("password", PASSWORD);
      settings.addProperty("httpsCaPem", CryptoTest.pem(CryptoTest.issuer));
      Report report = new Report("network", "TEST", "v3");
      PublicHttp http =
          new PublicHttp(
              report,
              host -> new InetAddress[] {publicIpUnchecked()},
              NetworkOptions.read(settings),
              System::nanoTime);
      assertEquals("ok", http.get(TARGET.toString()).text());
      assertFalse(
          peer.get(8, TimeUnit.SECONDS).toLowerCase(Locale.ROOT).contains("proxy-authorization"));
      JsonObject trace = report.data.getAsJsonArray("networkTraces").get(0).getAsJsonObject();
      assertTrue(trace.has("tls"));
      assertEquals(scheme.equals("https"), trace.has("proxyTls"));
      assertTrue(trace.getAsJsonObject("tls").getAsJsonArray("certificates").size() > 0);
      String encoded = Main.JSON.toJson(report);
      assertFalse(encoded.contains(USER));
      assertFalse(encoded.contains(PASSWORD));
      assertFalse(
          encoded.contains(
              Base64.getEncoder()
                  .encodeToString((USER + ":" + PASSWORD).getBytes(StandardCharsets.UTF_8))));
    }
  }

  @Test
  void proxyDoesNotBypassThePublicDestinationPolicy() throws Exception {
    try (ServerSocket listener = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
      listener.setSoTimeout(100);
      Report report = new Report("network", "PROD", "v3");
      PublicHttp http =
          new PublicHttp(
              report,
              host -> new InetAddress[] {InetAddress.getLoopbackAddress()},
              options(listener.getLocalPort(), "http"),
              System::nanoTime);
      NetworkFailure failure =
          assertThrows(
              NetworkFailure.class, () -> http.pki("http://target.example/list.crl", null));
      assertEquals("TARGET_BLOCKED", failure.code());
      assertThrows(SocketTimeoutException.class, listener::accept);
    }
  }

  @Test
  void authenticationAndMalformedTunnelResponsesHaveSpecificSafeFailures() throws Exception {
    for (boolean authenticated : List.of(false, true)) {
      NetworkFailure failure =
          assertThrows(
              NetworkFailure.class,
              () ->
                  ProxyConnection.tunnel(
                      new ByteArrayInputStream(
                          "HTTP/1.1 407 Proxy authentication required\r\nProxy-Authenticate: Basic realm=fixture\r\n\r\n"
                              .getBytes(StandardCharsets.US_ASCII)),
                      new NetworkBudget(System::nanoTime),
                      authenticated));
      assertEquals(authenticated ? "PROXY_AUTH_FAILED" : "PROXY_AUTH_REQUIRED", failure.code());
    }
    NetworkFailure unsupported =
        assertThrows(
            NetworkFailure.class,
            () ->
                ProxyConnection.tunnel(
                    new ByteArrayInputStream(
                        "HTTP/1.1 407 Authentication required\r\nProxy-Authenticate: Negotiate\r\n\r\n"
                            .getBytes(StandardCharsets.US_ASCII)),
                    new NetworkBudget(System::nanoTime),
                    true));
    assertEquals("PROXY_AUTH_UNSUPPORTED", unsupported.code());
    assertThrows(
        NetworkFailure.class,
        () ->
            ProxyConnection.tunnel(
                new ByteArrayInputStream("INVALID\r\n".getBytes(StandardCharsets.US_ASCII)),
                new NetworkBudget(System::nanoTime),
                false));
    assertThrows(
        NetworkFailure.class,
        () ->
            ProxyConnection.tunnel(
                new ByteArrayInputStream(
                    "HTTP/1.1 403 Blocked\r\n\r\n".getBytes(StandardCharsets.US_ASCII)),
                new NetworkBudget(System::nanoTime),
                false));
  }

  @ParameterizedTest
  @ValueSource(
      strings = {
        "http://user:secret@proxy.example:8080",
        "file:///tmp/proxy",
        "http://proxy.example:0",
        "http://proxy.example:70000",
        "http://proxy.example/path",
        "http://proxy.example/?password=secret"
      })
  void invalidProxyAddressesDoNotEchoCredentials(String url) {
    JsonObject input = new JsonObject();
    input.addProperty("mode", "manual");
    input.addProperty("proxyUrl", url);
    var failure = assertThrows(IllegalArgumentException.class, () -> NetworkOptions.read(input));
    assertFalse(failure.getMessage().contains("secret"));
  }
}
