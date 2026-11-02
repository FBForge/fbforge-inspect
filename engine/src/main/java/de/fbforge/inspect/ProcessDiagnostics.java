package de.fbforge.inspect;

import static de.fbforge.inspect.Api.submission;
import static de.fbforge.inspect.JsonFields.*;

import com.google.gson.*;
import java.io.IOException;
import java.util.*;

/** Public process-message search and explicit destination-pair limits. */
final class ProcessDiagnostics {
  private final Api api;
  private final Report report;

  ProcessDiagnostics(Api api) {
    this.api = api;
    this.report = api.report;
  }

  public void process(JsonObject input) throws IOException {
    if (!report.apiVersion.equals("v3"))
      throw new IllegalArgumentException("Prozesssuche benötigt v3.");
    String model = value(input, "processModelId"),
        message = value(input, "messageId"),
        region = value(input, "region");
    if (model.isBlank()
        || model.length() > 512
        || message.isBlank()
        || message.length() > 255
        || !region.matches("DE[0-9]{0,12}"))
      throw new IllegalArgumentException(
          "Prozessmodell, Nachricht und Region DE mit bis zu 12 Ziffern erwartet.");
    String url =
        submission(report.environment)
            + "/v3/search-destinations/by-process-message?processModelId="
            + encode(model)
            + "&messageId="
            + encode(message)
            + "&region="
            + region;
    JsonArray destinations = api.pages(url, "destinations");
    report.data.add("destinations", destinations);
    int index = 0;
    for (JsonElement item : destinations) {
      JsonObject destination = item.getAsJsonObject();
      boolean matched = false;
      for (JsonElement element : array(destination, "processMessages")) {
        JsonObject context = element.getAsJsonObject();
        if (value(context, "processModelId").equals(model)
            && value(context, "messageId").equals(message)
            && regionMatches(array(context, "regions"), region)
            && schemaMatch(array(context, "schemas"), value(input, "schema"), value(input, "mime")))
          matched = true;
      }
      report.check(
          matched,
          "Prozesskontext passt",
          value(destination, "destinationId")
              + ": Modell, Nachricht, Region und optionale Schemainformation müssen im selben"
              + " Nachrichtenkontext passen.",
          "$.destinations[" + (index++) + "].processMessages",
          url);
    }
    String from = value(input, "fromDestinationId"), to = value(input, "toDestinationId");
    if (!from.isBlank() || !to.isBlank()) limits(input);
  }

  public void limits(JsonObject input) throws IOException {
    if (!report.apiVersion.equals("v3"))
      throw new IllegalArgumentException("Limits-Abfrage benötigt v3.");
    String from = uuid(value(input, "fromDestinationId")),
        to = uuid(value(input, "toDestinationId"));
    String url = submission(report.environment) + "/v3/limits/" + from + "/" + to;
    JsonObject limits = api.json(url);
    if (limits != null) {
      report.data.add("limits", limits);
      report.data.addProperty("fromDestinationId", from);
      report.data.addProperty("toDestinationId", to);
      report.add(
          "confirmed",
          "Größenlimits veröffentlicht",
          "Limits für das gewählte Absender- und Empfänger-Paar abgerufen.",
          "$.limits",
          url,
          "",
          "Eine öffentliche Limits-Antwort beweist keine erfolgreiche Einreichung.");
    }
  }
}
