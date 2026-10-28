package de.fbforge.inspect;

import com.google.gson.*;
import java.io.*;
import java.nio.charset.StandardCharsets;

/** One request per process makes cancellation and destruction of session data explicit. */
public final class Main {
  static final Gson JSON = new GsonBuilder().disableHtmlEscaping().create();

  public static void main(String[] args) throws Exception {
    JsonObject response;
    try {
      byte[] bytes = System.in.readNBytes(2 * 1024 * 1024 + 1);
      if (bytes.length > 2 * 1024 * 1024)
        throw new IllegalArgumentException("Eingabe überschreitet 2 MiB.");
      JsonObject request =
          JsonParser.parseString(new String(bytes, StandardCharsets.UTF_8)).getAsJsonObject();
      if (!request.has("protocolVersion") || request.get("protocolVersion").getAsInt() != 1)
        throw new IllegalArgumentException("IPC-Version nicht unterstützt.");
      Report report = run(request);
      response = new JsonObject();
      response.addProperty("ok", true);
      response.add("report", JSON.toJsonTree(report));
    } catch (Throwable e) {
      response = new JsonObject();
      response.addProperty("ok", false);
      // Parser/network errors may include input values. Never serialize their messages or stack
      // traces.
      response.addProperty(
          "error",
          e instanceof IllegalArgumentException && !e.getClass().getName().startsWith("com.google")
              ? safeMessage(e)
              : "Eingabe oder Antwort nicht auswertbar. Format, öffentliche Erreichbarkeit und"
                  + " Quelle prüfen.");
    }
    // The IPC contract is UTF-8 even when Windows chooses a different stdout charset.
    System.out.write((JSON.toJson(response) + "\n").getBytes(StandardCharsets.UTF_8));
    System.out.flush();
  }

  static String safeMessage(Throwable e) {
    String m = e.getMessage();
    // Only our domain messages are safe. Third-party IllegalArgumentExceptions can contain inputs.
    if (e.getStackTrace().length > 0
        && e.getStackTrace()[0].getClassName().startsWith("de.fbforge.inspect.")
        && m != null
        && m.length() < 250) return m;
    return "Eingabe nicht zulässig oder Format ungültig.";
  }

  public static Report run(JsonObject request) throws Exception {
    String operation = Api.value(request, "operation"),
        env = Api.value(request, "environment"),
        version = Api.value(request, "apiVersion");
    Api.submission(env);
    if (!version.equals("v2") && !version.equals("v3"))
      throw new IllegalArgumentException("API-Version muss v2 oder v3 sein.");
    JsonObject input = request.getAsJsonObject("input");
    if (input == null) throw new IllegalArgumentException("Eingabe fehlt.");
    String text = Api.value(input, "text");
    if (KeyDiagnostics.hasPrivateMaterial(text, operation.equals("keys"))
        || Api.value(input, "trustPem").contains("PRIVATE KEY"))
      throw new IllegalArgumentException(
          "Private oder symmetrische Schlüssel sind nicht zulässig. Eingabe verworfen.");
    Report report = new Report(operation, env, version);
    PublicHttp http = new PublicHttp(report);
    Api api = new Api(report, http::get);
    try {
      switch (operation) {
        case "destination" -> api.destination(input, http);
        case "routing" -> api.routes(input);
        case "areas" -> api.areas(input);
        case "process" -> api.process(input);
        case "keys" -> {
          if (!Api.value(input, "url").isBlank()) {
            var response = http.get(Api.value(input, "url"));
            if (!response.ok())
              report.unknown(
                  "Schlüsselabruf nicht erfolgreich",
                  "HTTP " + response.status(),
                  "$",
                  response.uri().toString());
            else KeyDiagnostics.inspect(response.text(), report, http, input);
          } else KeyDiagnostics.inspect(text, report, http, input);
        }
        case "schema" -> {
          if (!Api.value(input, "url").isBlank()) {
            var response = http.get(Api.value(input, "url"));
            if (!response.ok())
              report.unknown(
                  "Schemaabruf nicht erfolgreich",
                  "HTTP " + response.status(),
                  "$",
                  response.uri().toString());
            else {
              report.data.addProperty("schemaSource", response.uri().toString());
              Schemas.inspect(response.text(), report);
            }
          } else Schemas.inspect(text, report);
        }
        default -> throw new IllegalArgumentException("Unbekannte Diagnosefunktion.");
      }
    } catch (IOException e) {
      report.unknown(
          "Öffentlicher Abruf fehlgeschlagen",
          "Netzwerkziel, DNS, TLS oder Abfragebudget prüfen. Bereits geladene Daten bleiben"
              + " Teilbestand.",
          "$",
          "");
    }
    return report;
  }
}
