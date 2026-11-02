package de.fbforge.inspect;

import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.time.*;
import java.util.*;
import java.util.function.LongSupplier;

/** Validates every destination before the pinned-IP transport, including redirects. */
public final class PublicHttp implements PkiFetch {
  public static final int MAX_BODY = 4 * 1024 * 1024;

  @FunctionalInterface
  interface Resolver {
    InetAddress[] resolve(String host) throws IOException;
  }

  @FunctionalInterface
  interface Exchange {
    Response exchange(URI uri, InetAddress ip, byte[] post, NetworkBudget budget)
        throws IOException;
  }

  private final Report report;
  private final Resolver resolver;
  private final Exchange exchange;
  private final NetworkBudget budget;
  private final boolean viaProxy;

  public record Response(int status, Map<String, String> headers, byte[] body, URI uri) {
    public String text() {
      return new String(body, StandardCharsets.UTF_8);
    }

    public boolean ok() {
      return status >= 200 && status < 300;
    }
  }

  public PublicHttp(Report report) {
    this(report, NetworkOptions.direct());
  }

  PublicHttp(Report report, NetworkOptions network) {
    this(report, InetAddress::getAllByName, network, System::nanoTime);
  }

  PublicHttp(Report report, Resolver resolver, NetworkOptions network, LongSupplier clock) {
    this(
        report,
        resolver,
        (uri, ip, post, budget) -> PinnedHttp.exchange(uri, ip, post, budget, network),
        clock,
        network.manual());
  }

  PublicHttp(Report report, Resolver resolver, Exchange exchange, LongSupplier clock) {
    this(report, resolver, exchange, clock, false);
  }

  private PublicHttp(
      Report report, Resolver resolver, Exchange exchange, LongSupplier clock, boolean viaProxy) {
    this.report = report;
    this.resolver = resolver;
    this.exchange = exchange;
    this.budget = new NetworkBudget(clock);
    this.viaProxy = viaProxy;
  }

  public static boolean isPublic(InetAddress address) {
    if (address.isAnyLocalAddress()
        || address.isLoopbackAddress()
        || address.isLinkLocalAddress()
        || address.isSiteLocalAddress()
        || address.isMulticastAddress()) return false;
    byte[] a = address.getAddress();
    if (a.length == 4) {
      int x = a[0] & 255, y = a[1] & 255, z = a[2] & 255;
      return !(x == 0
          || x == 10
          || x == 127
          || x >= 224
          || (x == 169 && y == 254)
          || (x == 100 && y >= 64 && y <= 127)
          || (x == 172 && y >= 16 && y <= 31)
          || (x == 192 && y == 168)
          || (x == 192 && y == 0)
          || (x == 192 && y == 2)
          || (x == 198 && (y == 18 || y == 19))
          || (x == 198 && y == 51 && z == 100)
          || (x == 203 && y == 0 && z == 113));
    }
    // Only global-unicast IPv6. Exclude documentation and transition mechanisms.
    return (a[0] & 0xe0) == 0x20
        && !((a[0] & 255) == 0x20
            && (a[1] & 255) == 0x01
            && ((a[2] & 255) == 0x0d && (a[3] & 255) == 0xb8 || (a[2] & 255) == 0))
        && !((a[0] & 255) == 0x20 && (a[1] & 255) == 0x02);
  }

  public static void validateUri(URI uri, boolean pki) {
    if (uri.getHost() == null
        || uri.getUserInfo() != null
        || uri.getFragment() != null
        || !("https".equals(uri.getScheme()) || pki && "http".equals(uri.getScheme()))
        || uri.getPort() != -1 && uri.getPort() != ("https".equals(uri.getScheme()) ? 443 : 80))
      throw new IllegalArgumentException(
          "Nur öffentliche HTTP(S)-Standardports ohne Zugangsdaten sind zulässig.");
    if (uri.toString().length() > 8192)
      throw new IllegalArgumentException("URL überschreitet das Längenlimit.");
    if (uri.getRawQuery() != null)
      for (String part : uri.getRawQuery().split("&")) {
        String name =
            URLDecoder.decode(part.split("=", 2)[0], StandardCharsets.UTF_8)
                .toLowerCase(Locale.ROOT);
        if (Set.of(
                "access_token",
                "token",
                "api_key",
                "apikey",
                "secret",
                "password",
                "authorization",
                "client_secret",
                "x-amz-credential",
                "x-amz-signature")
            .contains(name))
          throw new IllegalArgumentException("URLs mit Zugangsdaten sind nicht zulässig.");
      }
  }

  public Response get(String url) throws IOException {
    return request(URI.create(url), false, null);
  }

  public Response pki(String url, byte[] ocsp) throws IOException {
    return request(URI.create(url), true, ocsp);
  }

  private Response request(URI uri, boolean pki, byte[] post) throws IOException {
    for (int redirects = 0; redirects <= 4; redirects++) {
      validateUri(uri, pki);
      budget.startRequest();
      String at = Instant.now().toString();
      long started = System.nanoTime();
      try {
        InetAddress[] addresses = resolver.resolve(uri.getHost());
        if (addresses.length == 0 || Arrays.stream(addresses).anyMatch(a -> !isPublic(a)))
          throw new NetworkFailure("TARGET_BLOCKED");
        budget.checkTime();
        Response response = exchange.exchange(uri, addresses[0], post, budget);
        report.requests.add(
            new Report.Request(
                uri.toString(),
                post == null ? "GET" : "POST",
                at,
                response.status,
                response.body.length,
                "HTTP",
                (System.nanoTime() - started) / 1_000_000,
                viaProxy));
        // Preserve only public, successful bodies. Key content is checked before retention.
        if (!pki && response.ok() && !KeyDiagnostics.hasPrivateMaterial(response.text()))
          report.rawResponses.put(uri.toString(), response.text());
        if (Set.of(301, 302, 303, 307, 308).contains(response.status)) {
          if (redirects == 4 || post != null)
            throw new IOException("Umleitungslimit oder OCSP-Umleitung blockiert.");
          URI next = uri.resolve(response.headers.getOrDefault("location", ""));
          if ("https".equals(uri.getScheme()) && !"https".equals(next.getScheme()))
            throw new IOException("TLS-Downgrade blockiert.");
          uri = next;
          continue;
        }
        return response;
      } catch (IOException e) {
        NetworkFailure failure =
            e instanceof NetworkFailure known
                ? known
                : new NetworkFailure(
                    e instanceof UnknownHostException
                        ? "DNS_FAILED"
                        : e instanceof SocketTimeoutException ? "READ_TIMEOUT" : "NETWORK_FAILED",
                    e);
        report.requests.add(
            new Report.Request(
                uri.toString(),
                post == null ? "GET" : "POST",
                at,
                0,
                0,
                failure.code(),
                (System.nanoTime() - started) / 1_000_000,
                viaProxy));
        throw failure;
      }
    }
    throw new IOException("Umleitungslimit erreicht.");
  }
}
