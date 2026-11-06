package de.fbforge.inspect;

import java.io.IOException;

/** Fixed diagnostic codes keep provider exceptions and credentials out of exported evidence. */
final class NetworkFailure extends IOException {
  private final String code;

  NetworkFailure(String code) {
    super("Netzwerkprüfung: " + code);
    this.code = code;
  }

  NetworkFailure(String code, IOException cause) {
    super("Netzwerkprüfung: " + code, cause);
    this.code = code;
  }

  String code() {
    return code;
  }

  String description() {
    if (code.startsWith("PROXY_TLS_") && !code.equals("PROXY_TLS_FAILED"))
      return "HTTPS-Proxy: " + new NetworkFailure(code.substring("PROXY_".length())).description();
    return switch (code) {
      case "DNS_FAILED" -> "Der öffentliche Zielhost konnte nicht per DNS aufgelöst werden.";
      case "TARGET_BLOCKED" -> "Das Ziel wurde als nicht öffentliche Netzwerkadresse blockiert.";
      case "CONNECT_TIMEOUT" -> "Die Verbindung zum Ziel hat das Zeitlimit überschritten.";
      case "CONNECT_FAILED" -> "Die TCP-Verbindung zum Ziel konnte nicht hergestellt werden.";
      case "TLS_FAILED" -> "TLS-Verbindung oder Prüfung des Serverzertifikats fehlgeschlagen.";
      case "TLS_EXPIRED" -> "Ein Zertifikat der HTTPS-Verbindung ist abgelaufen.";
      case "TLS_NOT_YET_VALID" ->
          "Ein Zertifikat der HTTPS-Verbindung ist noch nicht gültig. Auch die Systemzeit prüfen.";
      case "TLS_HOSTNAME" -> "Das Serverzertifikat passt nicht zum angefragten Hostnamen.";
      case "TLS_TRUST_UNKNOWN" ->
          "Die HTTPS-Zertifikatskette führt zu keinem verfügbaren Vertrauensanker. Unternehmens-CA"
              + " und Java-Vertrauensspeicher prüfen.";
      case "TLS_CERTIFICATE_INVALID" ->
          "Die Zertifikatskette der HTTPS-Verbindung konnte nicht bestätigt werden.";
      case "TLS_HANDSHAKE_FAILED" ->
          "Die TLS-Aushandlung wurde abgebrochen. Protokoll, Gegenstelle und Netzwerkweg prüfen.";
      case "PROXY_DNS_FAILED" -> "Der eingetragene Proxy konnte nicht per DNS aufgelöst werden.";
      case "PROXY_CONNECT_TIMEOUT" -> "Die Verbindung zum Proxy hat das Zeitlimit überschritten.";
      case "PROXY_CONNECT_FAILED" ->
          "Die TCP-Verbindung zum Proxy konnte nicht hergestellt werden.";
      case "PROXY_TLS_FAILED" ->
          "Die TLS-Verbindung zum HTTPS-Proxy konnte nicht bestätigt werden.";
      case "PROXY_AUTH_REQUIRED" -> "Der Proxy verlangt einen Benutzernamen und ein Passwort.";
      case "PROXY_AUTH_FAILED" -> "Die Proxy-Anmeldung wurde zurückgewiesen.";
      case "PROXY_AUTH_UNSUPPORTED" ->
          "Der Proxy bietet keine unterstützte Basic-Anmeldung an. Windows-Anmeldung und PAC werden"
              + " noch nicht unterstützt.";
      case "PROXY_CONNECT_REJECTED" ->
          "Der Proxy hat den Tunnel zur geprüften öffentlichen Ziel-IP abgelehnt.";
      case "PROXY_RESPONSE_INVALID" -> "Der Proxy hat keine auswertbare HTTP-Antwort geliefert.";
      case "READ_TIMEOUT" -> "Das Warten auf die Antwort hat das Zeitlimit überschritten.";
      default -> "Der öffentliche Abruf ist fehlgeschlagen oder das Abfragebudget wurde erreicht.";
    };
  }

  static String tlsCode(Throwable failure) {
    for (Throwable cause = failure; cause != null; cause = cause.getCause()) {
      if (cause instanceof java.security.cert.CertificateExpiredException) return "TLS_EXPIRED";
      if (cause instanceof java.security.cert.CertificateNotYetValidException)
        return "TLS_NOT_YET_VALID";
      if (cause instanceof java.security.cert.CertPathBuilderException) return "TLS_TRUST_UNKNOWN";
      if (cause instanceof java.security.cert.CertificateException
          && cause.getMessage() != null
          && (cause.getMessage().contains("No subject alternative")
              || cause.getMessage().contains("No name matching"))) return "TLS_HOSTNAME";
      if (cause instanceof java.security.cert.CertPathValidatorException invalid) {
        if (invalid.getReason()
            == java.security.cert.CertPathValidatorException.BasicReason.EXPIRED)
          return "TLS_EXPIRED";
        if (invalid.getReason()
            == java.security.cert.CertPathValidatorException.BasicReason.NOT_YET_VALID)
          return "TLS_NOT_YET_VALID";
      }
    }
    for (Throwable cause = failure; cause != null; cause = cause.getCause())
      if (cause instanceof java.security.cert.CertificateException)
        return "TLS_CERTIFICATE_INVALID";
    return failure instanceof javax.net.ssl.SSLHandshakeException
        ? "TLS_HANDSHAKE_FAILED"
        : "TLS_FAILED";
  }
}
