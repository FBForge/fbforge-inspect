package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.util.*;
import org.junit.jupiter.api.Test;

/** Whole public workflows, including their optional branches and failure boundaries. */
class WorkflowContractTest {
  private static final String FROM = "00000000-0000-4000-8000-000000000001";
  private static final String TO = "00000000-0000-4000-8000-000000000002";

  private static JsonObject processInput() {
    JsonObject input = JsonParser.parseString("""
        {"processModelId":"bpmn:fixture","messageId":"M/1","region":"DE091620000000",
         "schema":"urn:fixture:schema","mime":"application/json"}
        """).getAsJsonObject();
    input.addProperty("fromDestinationId", FROM);
    input.addProperty("toDestinationId", TO);
    return input;
  }

  private static String searchResponse() {
    return """
        {"count":1,"offset":0,"totalCount":1,"destinations":[
          {"destinationId":"fixture","processMessages":[
            {"processModelId":"bpmn:fixture","messageId":"M/1","regions":["DE09"],
             "schemas":[{"schemaUri":"urn:fixture:schema","mimeType":"application/json"}]}
          ]}
        ]}
        """;
  }

  @Test
  void destinationComparisonKeepsBothVersionsWithoutApplyingV3RulesToV2() throws Exception {
    Report report = new Report("destination", "TEST", "v2");
    List<String> calls = new ArrayList<>();
    Api api = new Api(report, url -> {
      calls.add(url);
      assertTrue(url.endsWith("/destinations/" + FROM));
      if (url.contains("/v2/")) return ApiTest.response(200,
          "{\"destinationId\":\"" + FROM + "\",\"status\":\"active\",\"metadataVersions\":[\"1.0.0\"],\"publicServices\":[]}");
      assertTrue(url.contains("/v3/"));
      return ApiTest.response(200, "{\"destinationId\":\"" + FROM + "\",\"type\":\"C\"}");
    });
    JsonObject input = new JsonObject();
    input.addProperty("destinationId", FROM);
    input.addProperty("compare", true);
    api.destination(input, null);
    assertEquals(2, calls.size());
    assertFalse(report.data.getAsJsonObject("destination").has("type"));
    assertEquals("C", report.data.getAsJsonObject("comparisonv3").get("type").getAsString());
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("Versionsvergleich") && f.status().equals("notApplicable")));
    assertFalse(report.findings.stream().anyMatch(f -> f.title().equals("Zustellpunkttyp") && f.status().equals("error")));
  }

  @Test
  void processSearchAndPairLimitsPreserveTheDocumentedApplicableValues() throws Exception {
    Report report = new Report("process", "STAGE", "v3");
    List<String> calls = new ArrayList<>();
    JsonObject limits = JsonParser.parseString("""
        {"attachments":{
          "platform":{"maxNumber":100,"maxSizeIndividualBytes":70000000,"maxSizeAggregateBytes":999999999},
          "receive":{"maxNumber":80,"maxSizeIndividualBytes":90000000,"maxSizeAggregateBytes":90000001},
          "send":{"maxNumber":15,"maxSizeIndividualBytes":80000000,"maxSizeAggregateBytes":90000002},
          "applicable":{"maxNumber":15,"maxSizeIndividualBytes":70000000,"maxSizeAggregateBytes":90000001}},
         "additionalSourceField":"preserved"}
        """).getAsJsonObject();
    Api api = new Api(report, url -> {
      calls.add(url);
      assertTrue(url.startsWith("https://stage.fit-connect.fitko.net/submission-api/v3/"));
      if (url.contains("/search-destinations/")) {
        assertTrue(url.contains("processModelId=bpmn%3Afixture&messageId=M%2F1&region=DE091620000000"));
        return ApiTest.response(200, searchResponse());
      }
      assertTrue(url.endsWith("/limits/" + FROM + "/" + TO));
      return ApiTest.response(200, limits.toString());
    });
    api.process(processInput());
    assertEquals(2, calls.size());
    assertEquals(limits, report.data.getAsJsonObject("limits"));
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("Prozesskontext passt") && f.status().equals("confirmed")));
  }

  @Test
  void unavailableLimitsDoNotEraseTheSuccessfulProcessSearchOrBecomeEmptySuccess() throws Exception {
    Report report = new Report("process", "TEST", "v3");
    Api api = new Api(report, url -> url.contains("/search-destinations/")
        ? ApiTest.response(200, searchResponse()) : ApiTest.response(429, "{}"));
    api.process(processInput());
    assertEquals(1, report.data.getAsJsonArray("destinations").size());
    assertFalse(report.data.has("limits"));
    assertTrue(report.findings.stream().anyMatch(f -> f.status().equals("unknown") && f.detail().contains("HTTP 429")));
  }

  @Test
  void v2ProcessRequestsAreRejectedBeforeAnyNetworkRequest() {
    Api api = new Api(new Report("process", "TEST", "v2"), url -> {
      fail("v2 process search must not issue a request");
      return null;
    });
    assertThrows(IllegalArgumentException.class, () -> api.process(processInput()));
  }
}
