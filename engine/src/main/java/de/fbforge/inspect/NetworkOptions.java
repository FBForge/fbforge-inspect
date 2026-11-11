package de.fbforge.inspect;

import com.google.gson.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Operator-selected proxy configuration. Never place this object in a report or log. */
final class NetworkOptions {
  private final URI proxy;
  private final String username;
  private final String password;
  final String httpsCaPem;

  private NetworkOptions(URI proxy, String username, String password) {
    this(proxy, username, password, "");
  }

  private NetworkOptions(URI proxy, String username, String password, String httpsCaPem) {
    this.proxy = proxy;
    this.username = username;
    this.password = password;
    this.httpsCaPem = httpsCaPem;
  }

  static NetworkOptions direct() {
    return new NetworkOptions(null, "", "");
  }

  static NetworkOptions read(JsonObject options) {
    if (options == null) return direct();
    String ca = JsonFields.value(options, "httpsCaPem");
    if (!ca.isBlank()) HttpsTrust.factory(ca);
    if (!options.has("mode") || JsonFields.value(options, "mode").equals("direct"))
      return new NetworkOptions(null, "", "", ca);
    if (!JsonFields.value(options, "mode").equals("manual"))
      throw new IllegalArgumentException("Netzwerkmodus muss direkt oder manuell sein.");
    URI proxy;
    try {
      proxy = URI.create(JsonFields.value(options, "proxyUrl").strip());
    } catch (IllegalArgumentException invalid) {
      throw new IllegalArgumentException("Proxy-Adresse ist ungültig.");
    }
    if (proxy.getHost() == null
        || !Set.of("http", "https").contains(proxy.getScheme())
        || proxy.getUserInfo() != null
        || proxy.getQuery() != null
        || proxy.getFragment() != null
        || proxy.getPort() == 0
        || proxy.getPort() > 65535
        || !(proxy.getPath() == null || proxy.getPath().isEmpty() || proxy.getPath().equals("/"))
        || proxy.toString().length() > 2048)
      throw new IllegalArgumentException(
          "HTTP(S)-Proxy mit Host und Port ohne Pfad oder Zugangsdaten in der Adresse angeben.");
    String username = JsonFields.value(options, "username"),
        password = JsonFields.value(options, "password");
    if (username.length() > 512
        || password.length() > 4096
        || username.contains(":")
        || username.codePoints().anyMatch(Character::isISOControl)
        || password.codePoints().anyMatch(Character::isISOControl)
        || username.isEmpty() && !password.isEmpty())
      throw new IllegalArgumentException(
          "Proxy-Zugangsdaten sind nicht zulässig oder der Benutzername fehlt.");
    return new NetworkOptions(proxy, username, password, ca);
  }

  boolean manual() {
    return proxy != null;
  }

  URI proxy() {
    return proxy;
  }

  int proxyPort() {
    return proxy.getPort() < 0 ? (proxy.getScheme().equals("https") ? 443 : 80) : proxy.getPort();
  }

  String authorization() {
    return username.isEmpty()
        ? null
        : "Basic "
            + Base64.getEncoder()
                .encodeToString((username + ":" + password).getBytes(StandardCharsets.UTF_8));
  }

  @Override
  public String toString() {
    return manual() ? "Manual proxy configured" : "Direct connection";
  }
}
