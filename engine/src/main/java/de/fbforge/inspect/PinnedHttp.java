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
    return exchange(uri, ip, post, budget, NetworkOptions.direct());
  }

  static PublicHttp.Response exchange(
      URI uri, InetAddress ip, byte[] post, NetworkBudget budget, NetworkOptions network)
      throws IOException {
    return exchange(
        uri, ip, post, budget, network, (SSLSocketFactory) SSLSocketFactory.getDefault());
  }

  static PublicHttp.Response exchange(
      URI uri,
      InetAddress ip,
      byte[] post,
      NetworkBudget budget,
      NetworkOptions network,
      SSLSocketFactory tlsFactory)
      throws IOException {
    Socket connection =
        network.manual()
            ? ProxyConnection.open(uri, ip, network, budget, tlsFactory)
            : connect(uri, ip, budget);
    try {
      if (uri.getScheme().equals("https"))
        connection = secure(connection, uri, budget, "TLS_FAILED", tlsFactory);
      boolean forward = network.manual() && uri.getScheme().equals("http");
      writeRequest(
          connection.getOutputStream(),
          uri,
          post,
          forward ? "http://" + ProxyConnection.authority(ip) : "",
          forward ? network.authorization() : null);
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
      PublicHttp.Response response =
          HttpResponseReader.read(new BufferedInputStream(timed), uri, budget);
      if (network.manual() && response.status() == 407)
        throw new NetworkFailure(
            ProxyConnection.authenticationFailure(
                response.headers(), network.authorization() != null));
      return response;
    } finally {
      connection.close();
    }
  }

  private static Socket connect(URI uri, InetAddress ip, NetworkBudget budget) throws IOException {
    Socket socket = new Socket();
    try {
      socket.connect(
          new InetSocketAddress(ip, uri.getScheme().equals("https") ? 443 : 80),
          budget.timeoutMillis(8000));
      socket.setSoTimeout(budget.timeoutMillis(12000));
      return socket;
    } catch (IOException failure) {
      try {
        socket.close();
      } catch (IOException ignored) {
      }
      throw new NetworkFailure(
          failure instanceof SocketTimeoutException ? "CONNECT_TIMEOUT" : "CONNECT_FAILED",
          failure);
    }
  }

  static Socket secure(Socket socket, URI uri, NetworkBudget budget, String failureCode)
      throws IOException {
    return secure(
        socket, uri, budget, failureCode, (SSLSocketFactory) SSLSocketFactory.getDefault());
  }

  static Socket secure(
      Socket socket, URI uri, NetworkBudget budget, String failureCode, SSLSocketFactory factory)
      throws IOException {
    SSLSocket tls = null;
    try {
      tls =
          (SSLSocket)
              factory.createSocket(
                  socket, uri.getHost(), uri.getPort() < 0 ? 443 : uri.getPort(), true);
      configureTls(tls, uri);
      tls.setSoTimeout(budget.timeoutMillis(12000));
      tls.startHandshake();
      return tls;
    } catch (IOException failure) {
      if (tls != null)
        try {
          tls.close();
        } catch (IOException ignored) {
        }
      throw new NetworkFailure(failureCode, failure);
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
    writeRequest(out, uri, post, "", null);
  }

  static void writeRequest(
      OutputStream out, URI uri, byte[] post, String proxyPrefix, String proxyAuthorization)
      throws IOException {
    String path = uri.getRawPath();
    if (path == null || path.isEmpty()) path = "/";
    if (uri.getRawQuery() != null) path += "?" + uri.getRawQuery();
    String headers =
        (post == null ? "GET" : "POST")
            + " "
            + proxyPrefix
            + path
            + " HTTP/1.1\r\nHost: "
            + uri.getHost()
            + "\r\n"
            + "User-Agent: FBForge-Inspect/0.1.0\r\n"
            + "Accept: */*\r\n"
            + "Accept-Encoding: identity\r\n"
            + "Connection: close\r\n";
    if (proxyAuthorization != null)
      headers += "Proxy-Authorization: " + proxyAuthorization + "\r\n";
    if (post != null)
      headers +=
          "Content-Type: application/ocsp-request\r\nContent-Length: " + post.length + "\r\n";
    out.write((headers + "\r\n").getBytes(StandardCharsets.US_ASCII));
    if (post != null) out.write(post);
    out.flush();
  }
}
