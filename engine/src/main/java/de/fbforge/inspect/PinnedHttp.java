package de.fbforge.inspect;

import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import javax.net.ssl.*;

/** Connects only to the IP already approved by PublicHttp; TLS still authenticates the host. */
final class PinnedHttp {
  private PinnedHttp() {}

  static PublicHttp.Response exchange(URI uri, InetAddress ip, byte[] post, NetworkBudget budget)
      throws IOException {
    int port = "https".equals(uri.getScheme()) ? 443 : 80;
    try (Socket tcp = new Socket()) {
      tcp.connect(new InetSocketAddress(ip, port), budget.timeoutMillis(8000));
      tcp.setSoTimeout(budget.timeoutMillis(12000));
      Socket connection = tcp;
      if (port == 443) {
        SSLSocket tls =
            (SSLSocket)
                ((SSLSocketFactory) SSLSocketFactory.getDefault())
                    .createSocket(tcp, uri.getHost(), port, true);
        configureTls(tls, uri);
        tls.startHandshake();
        connection = tls;
      }
      try {
        writeRequest(connection.getOutputStream(), uri, post);
        Socket active = connection;
        InputStream timed =
            new FilterInputStream(connection.getInputStream()) {
              @Override
              public int read() throws IOException {
                active.setSoTimeout(budget.timeoutMillis(12000));
                return super.read();
              }

              @Override
              public int read(byte[] b, int offset, int length) throws IOException {
                active.setSoTimeout(budget.timeoutMillis(12000));
                return super.read(b, offset, length);
              }
            };
        return HttpResponseReader.read(new BufferedInputStream(timed), uri, budget);
      } finally {
        if (connection != tcp) connection.close();
      }
    }
  }

  static void configureTls(SSLSocket tls, URI uri) {
    SSLParameters parameters = tls.getSSLParameters();
    parameters.setEndpointIdentificationAlgorithm("HTTPS");
    if (!uri.getHost().contains(":"))
      parameters.setServerNames(List.of(new SNIHostName(uri.getHost())));
    tls.setSSLParameters(parameters);
  }

  static void writeRequest(OutputStream out, URI uri, byte[] post) throws IOException {
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
    out.write((headers + "\r\n").getBytes(StandardCharsets.US_ASCII));
    if (post != null) out.write(post);
    out.flush();
  }
}
