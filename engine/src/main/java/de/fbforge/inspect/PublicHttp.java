package de.fbforge.inspect;

import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.time.*;
import java.util.*;
import javax.net.ssl.*;

/** A pinned-address HTTP/1.1 client. DNS is checked once and the checked IP is connected. */
public final class PublicHttp {
  public static final int MAX_BODY = 4 * 1024 * 1024;
  private final Report report;
  private int count;
  private long total;
  private final long deadline = System.nanoTime() + Duration.ofSeconds(120).toNanos();

  public record Response(int status, Map<String, String> headers, byte[] body, URI uri) {
    public String text() {
      return new String(body, StandardCharsets.UTF_8);
    }

    public boolean ok() {
      return status >= 200 && status < 300;
    }
  }

  public PublicHttp(Report report) {
    this.report = report;
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
      if (++count > 40 || System.nanoTime() > deadline || total > 20L * 1024 * 1024)
        throw new IOException("Abfragebudget erreicht.");
      String at = Instant.now().toString();
      try {
        InetAddress[] addresses = InetAddress.getAllByName(uri.getHost());
        if (addresses.length == 0 || Arrays.stream(addresses).anyMatch(a -> !isPublic(a)))
          throw new IOException("Nicht öffentliches Netzwerkziel blockiert.");
        Response response = exchange(uri, addresses[0], post);
        total += response.body.length;
        if (total > 20L * 1024 * 1024) throw new IOException("Gesamtantwortbudget erreicht.");
        report.requests.add(
            new Report.Request(
                uri.toString(),
                post == null ? "GET" : "POST",
                at,
                response.status,
                response.body.length,
                "HTTP"));
        // Preserve only public, successful bodies. Key content is checked before retention.
        if (response.ok() && !KeyDiagnostics.hasPrivateMaterial(response.text()))
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
        report.requests.add(
            new Report.Request(
                uri.toString(),
                post == null ? "GET" : "POST",
                at,
                0,
                0,
                "Netzwerkzugriff fehlgeschlagen oder blockiert"));
        throw new IOException(
            "Öffentlicher Abruf fehlgeschlagen oder durch die Netzwerkrichtlinie blockiert.", e);
      }
    }
    throw new IOException("Umleitungslimit erreicht.");
  }

  private Response exchange(URI uri, InetAddress ip, byte[] post) throws IOException {
    int port = "https".equals(uri.getScheme()) ? 443 : 80;
    try (Socket tcp = new Socket()) {
      tcp.connect(new InetSocketAddress(ip, port), 8000);
      tcp.setSoTimeout(12000);
      Socket connection = tcp;
      if (port == 443) {
        SSLSocket tls =
            (SSLSocket)
                ((SSLSocketFactory) SSLSocketFactory.getDefault())
                    .createSocket(tcp, uri.getHost(), port, true);
        SSLParameters parameters = tls.getSSLParameters();
        parameters.setEndpointIdentificationAlgorithm("HTTPS");
        if (!uri.getHost().contains(":"))
          parameters.setServerNames(List.of(new SNIHostName(uri.getHost())));
        tls.setSSLParameters(parameters);
        tls.startHandshake();
        connection = tls;
      }
      String path = uri.getRawPath();
      if (path == null || path.isEmpty()) path = "/";
      if (uri.getRawQuery() != null) path += "?" + uri.getRawQuery();
      String headers =
          (post == null ? "GET" : "POST")
              + " "
              + path
              + " HTTP/1.1\r\nHost: "
              + uri.getHost()
              + "\r\n"
              + "User-Agent: FBForge-Inspect/0.1.0\r\n"
              + "Accept: */*\r\n"
              + "Accept-Encoding: identity\r\n"
              + "Connection: close\r\n";
      if (post != null)
        headers +=
            "Content-Type: application/ocsp-request\r\nContent-Length: " + post.length + "\r\n";
      OutputStream out = connection.getOutputStream();
      out.write((headers + "\r\n").getBytes(StandardCharsets.US_ASCII));
      if (post != null) out.write(post);
      out.flush();
      InputStream in = new BufferedInputStream(connection.getInputStream());
      String[] status = line(in).split(" ", 3);
      if (status.length < 2 || !status[0].startsWith("HTTP/1."))
        throw new IOException("Ungültige HTTP-Antwort.");
      Map<String, String> received = new TreeMap<>();
      String row;
      int size = 0;
      while (!(row = line(in)).isEmpty()) {
        size += row.length();
        if (size > 65536) throw new IOException("HTTP-Header zu groß.");
        int colon = row.indexOf(':');
        if (colon <= 0) throw new IOException("Ungültiger HTTP-Header.");
        String key = row.substring(0, colon).toLowerCase(Locale.ROOT),
            value = row.substring(colon + 1).trim();
        if (received.putIfAbsent(key, value) != null
            && Set.of("content-length", "transfer-encoding").contains(key))
          throw new IOException("Mehrdeutige HTTP-Länge.");
      }
      if (!received.getOrDefault("content-encoding", "identity").equals("identity"))
        throw new IOException("Komprimierte Antwort nicht zugelassen.");
      ByteArrayOutputStream body = new ByteArrayOutputStream();
      if (received.containsKey("transfer-encoding")) {
        if (!received.get("transfer-encoding").equalsIgnoreCase("chunked"))
          throw new IOException("Unbekannte Übertragungskodierung.");
        while (true) {
          String chunk = line(in).split(";", 2)[0];
          int length;
          try {
            length = Integer.parseInt(chunk, 16);
          } catch (NumberFormatException e) {
            throw new IOException("Ungültige Chunk-Länge.");
          }
          if (length == 0) break;
          copy(in, body, length);
          if (!line(in).isEmpty()) throw new IOException("Ungültiger Chunk-Abschluss.");
        }
      } else if (received.containsKey("content-length")) {
        try {
          copy(in, body, Integer.parseInt(received.get("content-length")));
        } catch (NumberFormatException e) {
          throw new IOException("Ungültige Inhaltslänge.");
        }
      } else {
        byte[] buf = new byte[8192];
        int n;
        while ((n = in.read(buf)) != -1) {
          checkBudget(body.size() + n);
          body.write(buf, 0, n);
        }
      }
      return new Response(Integer.parseInt(status[1]), received, body.toByteArray(), uri);
    }
  }

  private void checkBudget(int size) throws IOException {
    if (size < 0 || size > MAX_BODY || System.nanoTime() > deadline)
      throw new IOException("Antwort- oder Zeitlimit erreicht.");
  }

  private void copy(InputStream in, ByteArrayOutputStream out, int length) throws IOException {
    checkBudget(out.size() + length);
    if (length < 0) throw new IOException("Negative Länge.");
    byte[] buffer = new byte[8192];
    while (length > 0) {
      checkBudget(out.size());
      int n = in.read(buffer, 0, Math.min(length, buffer.length));
      if (n < 0) throw new EOFException();
      out.write(buffer, 0, n);
      length -= n;
    }
  }

  private static String line(InputStream in) throws IOException {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    int c;
    while ((c = in.read()) != -1) {
      if (c == '\n') return out.toString(StandardCharsets.US_ASCII).stripTrailing();
      if (out.size() > 8192) throw new IOException("HTTP-Zeile zu groß.");
      out.write(c);
    }
    throw new EOFException();
  }
}
