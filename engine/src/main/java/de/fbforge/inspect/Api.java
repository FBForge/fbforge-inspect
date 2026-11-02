package de.fbforge.inspect;

import com.google.gson.*;
import java.io.IOException;

/** Public diagnosis facade. Transport, pagination and workflow rules have separate owners. */
public final class Api {
  public static final String DOC =
      "https://docs.fitko.de/fit-connect/docs/sending/get-destination/";

  @FunctionalInterface
  public interface Fetch {
    PublicHttp.Response get(String url) throws IOException;
  }

  final Report report;
  private final PublicJsonClient client;

  public Api(Report report, Fetch fetch) {
    this.report = report;
    this.client = new PublicJsonClient(report, fetch);
  }

  public static String submission(String env) {
    return switch (env) {
      case "TEST" -> "https://test.fit-connect.fitko.dev/submission-api";
      case "STAGE" -> "https://stage.fit-connect.fitko.net/submission-api";
      case "PROD" -> "https://prod.fit-connect.fitko.net/submission-api";
      default -> throw new IllegalArgumentException("Unbekannte Umgebung.");
    };
  }

  public static String routing(String env) {
    return env.equals("TEST")
        ? "https://routing-api-testing.fit-connect.fitko.dev"
        : "https://routing-api-prod.fit-connect.fitko.net";
  }

  public JsonObject json(String url) throws IOException {
    return client.json(url);
  }

  public JsonArray pages(String url, String field) throws IOException {
    return client.pages(url, field);
  }

  public JsonObject destination(JsonObject input, PublicHttp http) throws IOException {
    return new DestinationDiagnostics(this).destination(input, http);
  }

  public static void inspectDestination(
      JsonObject destination, JsonObject input, Report report, String url) {
    DestinationDiagnostics.inspectDestination(destination, input, report, url);
  }

  void pvog(JsonObject input, Signatures signatures) throws IOException {
    new Pvog(this).inspect(input, signatures);
  }

  public void routes(JsonObject input) throws IOException {
    new RoutingDiagnostics(this).routes(input);
  }

  public void areas(JsonObject input) throws IOException {
    new RoutingDiagnostics(this).areas(input);
  }

  public void process(JsonObject input) throws IOException {
    new ProcessDiagnostics(this).process(input);
  }

  public void limits(JsonObject input) throws IOException {
    new ProcessDiagnostics(this).limits(input);
  }
}
