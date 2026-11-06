package de.fbforge.inspect;

import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Explicit operator proxy; CONNECT and absolute HTTP targets preserve the validated public IP. */
final class ProxyConnection {
  private ProxyConnection() {}

  static Socket open(
      URI destination, InetAddress validated, NetworkOptions options, NetworkBudget budget)
      throws IOException {
    return open(
        destination,
        validated,
        options,
        budget,
        (javax.net.ssl.SSLSocketFactory) javax.net.ssl.SSLSocketFactory.getDefault());
  }

  static Socket open(
      URI destination,
      InetAddress validated,
      NetworkOptions options,
      NetworkBudget budget,
      javax.net.ssl.SSLSocketFactory tlsFactory)
      throws IOException {
    Socket socket = new Socket();
    try {
      TransportTrace.stage("proxyDns");
      InetAddress proxy;
      try {
        proxy = InetAddress.getByName(options.proxy().getHost());
      } catch (UnknownHostException failure) {
        throw new NetworkFailure("PROXY_DNS_FAILED", failure);
      }
      TransportTrace.stage("proxyTcp");
      try {
        socket.connect(
            new InetSocketAddress(proxy, options.proxyPort()), budget.timeoutMillis(8000));
        socket.setSoTimeout(budget.timeoutMillis(12000));
      } catch (SocketTimeoutException failure) {
        throw new NetworkFailure("PROXY_CONNECT_TIMEOUT", failure);
      } catch (IOException failure) {
        throw new NetworkFailure("PROXY_CONNECT_FAILED", failure);
      }
      if (options.proxy().getScheme().equals("https")) {
        TransportTrace.stage("proxyTls");
        socket = PinnedHttp.secure(socket, options.proxy(), budget, "PROXY_TLS_FAILED", tlsFactory);
      }
      if (destination.getScheme().equals("https")) {
        TransportTrace.stage("proxyConnect");
        socket.getOutputStream().write(connectRequest(validated, options.authorization()));
        socket.getOutputStream().flush();
        tunnel(socket.getInputStream(), budget, options.authorization() != null);
      }
      return socket;
    } catch (IOException failure) {
      try {
        socket.close();
      } catch (IOException ignored) {
      }
      throw failure instanceof NetworkFailure
          ? failure
          : new NetworkFailure("PROXY_RESPONSE_INVALID", failure);
    }
  }

  static String authenticationFailure(Map<String, String> headers, boolean authenticated) {
    String challenge = headers.getOrDefault("proxy-authenticate", "").toLowerCase(Locale.ROOT);
    return !challenge.isEmpty() && !challenge.contains("basic")
        ? "PROXY_AUTH_UNSUPPORTED"
        : authenticated ? "PROXY_AUTH_FAILED" : "PROXY_AUTH_REQUIRED";
  }

  static String authority(InetAddress ip) {
    String host = ip.getHostAddress();
    return host.contains(":") ? "[" + host + "]" : host;
  }

  static byte[] connectRequest(InetAddress ip, String authorization) {
    String target = authority(ip) + ":443";
    return ("CONNECT "
            + target
            + " HTTP/1.1\r\nHost: "
            + target
            + "\r\n"
            + (authorization == null ? "" : "Proxy-Authorization: " + authorization + "\r\n")
            + "\r\n")
        .getBytes(StandardCharsets.US_ASCII);
  }

  static void tunnel(InputStream input, NetworkBudget budget, boolean authenticated)
      throws IOException {
    String line = HttpResponseReader.line(input, budget);
    if (!line.matches("HTTP/1\\.[01] [2-5][0-9]{2}(?: .*)?"))
      throw new NetworkFailure("PROXY_RESPONSE_INVALID");
    int status = Integer.parseInt(line.substring(9, 12)), size = 0;
    Map<String, String> headers = new HashMap<>();
    while (!(line = HttpResponseReader.line(input, budget)).isEmpty()) {
      size += line.length() + 2;
      if (size > 32768) throw new NetworkFailure("PROXY_RESPONSE_INVALID");
      Map.Entry<String, String> header = HttpResponseReader.header(line);
      headers.merge(header.getKey(), header.getValue(), (a, b) -> a + ", " + b);
    }
    if (status == 407) {
      throw new NetworkFailure(authenticationFailure(headers, authenticated));
    }
    if (status < 200 || status >= 300) throw new NetworkFailure("PROXY_CONNECT_REJECTED");
  }
}
