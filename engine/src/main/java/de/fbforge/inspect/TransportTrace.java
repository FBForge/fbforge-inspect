package de.fbforge.inspect;

import com.google.gson.*;
import java.security.MessageDigest;
import java.security.cert.X509Certificate;
import java.util.HexFormat;
import javax.net.ssl.SSLSocket;

/** One thread-local request trace. Only public peer data and fixed stage names are retained. */
final class TransportTrace {
  private static final ThreadLocal<TransportTrace> CURRENT = new ThreadLocal<>();
  final JsonObject data = new JsonObject();
  private final JsonArray phases = new JsonArray();
  private String stage;
  private long started;

  static TransportTrace begin(String url, boolean proxy) {
    TransportTrace trace = new TransportTrace();
    trace.data.addProperty("url", url);
    trace.data.addProperty("route", proxy ? "manualProxy" : "direct");
    trace.data.add("phases", trace.phases);
    CURRENT.set(trace);
    stage("dns");
    return trace;
  }

  static void stage(String name) {
    TransportTrace trace = CURRENT.get();
    if (trace == null) return;
    trace.end("confirmed");
    trace.stage = name;
    trace.started = System.nanoTime();
  }

  private void end(String outcome) {
    if (stage == null) return;
    JsonObject phase = new JsonObject();
    phase.addProperty("stage", stage);
    phase.addProperty("durationMs", (System.nanoTime() - started) / 1_000_000);
    phase.addProperty("outcome", outcome);
    phases.add(phase);
    stage = null;
  }

  static void peer(SSLSocket socket) {
    TransportTrace trace = CURRENT.get();
    if (trace == null) return;
    try {
      JsonObject tls = new JsonObject();
      tls.addProperty("protocol", socket.getSession().getProtocol());
      tls.addProperty("cipher", socket.getSession().getCipherSuite());
      JsonArray chain = new JsonArray();
      for (var certificate : socket.getSession().getPeerCertificates()) {
        if (!(certificate instanceof X509Certificate cert) || chain.size() >= 10) break;
        JsonObject item = new JsonObject();
        item.addProperty("subject", cert.getSubjectX500Principal().getName());
        item.addProperty("issuer", cert.getIssuerX500Principal().getName());
        item.addProperty("notAfter", cert.getNotAfter().toInstant().toString());
        item.addProperty(
            "sha256",
            HexFormat.of()
                .formatHex(MessageDigest.getInstance("SHA-256").digest(cert.getEncoded())));
        chain.add(item);
      }
      tls.add("certificates", chain);
      // The proxy and the target are separate TLS peers.
      trace.data.add(
          trace.stage != null && trace.stage.equals("proxyTls") ? "proxyTls" : "tls", tls);
    } catch (Exception ignored) {
      trace.data.addProperty("peerUnavailable", true);
    }
  }

  void finish(String outcome) {
    end(outcome);
    data.addProperty("outcome", outcome);
    CURRENT.remove();
  }
}
