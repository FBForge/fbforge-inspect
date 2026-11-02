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
    return switch (code) {
      case "DNS_FAILED" -> "Der öffentliche Zielhost konnte nicht per DNS aufgelöst werden.";
      case "TARGET_BLOCKED" -> "Das Ziel wurde als nicht öffentliche Netzwerkadresse blockiert.";
      case "CONNECT_TIMEOUT" -> "Die Verbindung zum Ziel hat das Zeitlimit überschritten.";
      case "CONNECT_FAILED" -> "Die TCP-Verbindung zum Ziel konnte nicht hergestellt werden.";
      case "TLS_FAILED" -> "TLS-Verbindung oder Prüfung des Serverzertifikats fehlgeschlagen.";
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
}
