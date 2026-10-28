package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.net.URI;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

class ApiTest {
  static PublicHttp.Response response(int status, String body) {
    return new PublicHttp.Response(
        status, Map.of(), body.getBytes(), URI.create("https://example.org"));
  }

  @Test
  void retainsRepeatedDestinationsAcrossPagesAndContexts() throws Exception {
    Report r = new Report("routing", "TEST", "v3");
    Api api =
        new Api(
            r,
            url ->
                url.endsWith("offset=0")
                    ? response(
                        200,
                        "{\"count\":1,\"offset\":0,\"totalCount\":2,\"routes\":[{\"destinationId\":\"same\",\"organisation\":\"A\"}]}")
                    : response(
                        200,
                        "{\"count\":1,\"offset\":1,\"totalCount\":2,\"routes\":[{\"destinationId\":\"same\",\"organisation\":\"B\"}]}"));
    assertEquals(2, api.pages("https://example.org/routes", "routes").size());
    assertEquals("confirmed", r.findings.getLast().status());
  }

  @Test
  void partialFailureIsNotAnEmptySuccess() throws Exception {
    for (int status : new int[] {403, 404, 429}) {
      Report r = new Report("routing", "TEST", "v3");
      Api api = new Api(r, url -> response(status, "{}"));
      assertTrue(api.pages("https://example.org/routes", "routes").isEmpty());
      assertEquals("unknown", r.findings.getLast().status());
    }
  }

  @Test
  void doesNotCombineSchemasFromDifferentServices() {
    JsonObject d =
        JsonParser.parseString(
                "{\"status\":\"active\",\"publicServices\":[{\"identifier\":\"a\",\"submissionSchemas\":[{\"schemaUri\":\"schema-a\",\"mimeType\":\"application/json\"}]},{\"identifier\":\"b\",\"submissionSchemas\":[{\"schemaUri\":\"schema-b\",\"mimeType\":\"application/xml\"}]}]}")
            .getAsJsonObject();
    JsonObject input = new JsonObject();
    input.addProperty("service", "a");
    input.addProperty("schema", "schema-b");
    Report r = new Report("destination", "TEST", "v3");
    Api.inspectDestination(d, input, r, "fixture");
    assertTrue(
        r.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("Erwartete Verwaltungskonfiguration")
                        && f.status().equals("error")));
  }

  @Test
  void xsdExternalEntitiesAreRejected() {
    assertThrows(
        Exception.class,
        () ->
            Schemas.inspect(
                "<!DOCTYPE schema [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><schema"
                    + " xmlns='http://www.w3.org/2001/XMLSchema'>&x;</schema>",
                new Report("schema", "TEST", "v3")));
  }

  @Test
  void referencesRemainVisibleAndUnresolved() throws Exception {
    Report r = new Report("schema", "TEST", "v3");
    Schemas.inspect("{\"$ref\":\"https://example.org/other.json\"}", r);
    assertEquals(1, r.data.getAsJsonArray("references").size());
    assertEquals("unknown", r.findings.getLast().status());
  }

  @Test
  void networkFailureAfterFirstPageRetainsPartialContexts() throws Exception {
    Report r = new Report("routing", "TEST", "v3");
    Api api =
        new Api(
            r,
            url -> {
              if (url.endsWith("offset=0"))
                return response(
                    200,
                    "{\"count\":1,\"offset\":0,\"totalCount\":2,\"routes\":[{\"destinationId\":\"first\"}]}");
              throw new java.io.IOException("synthetic timeout");
            });
    assertEquals(1, api.pages("https://example.org/routes", "routes").size());
    assertEquals("unknown", r.findings.getLast().status());
  }

  @Test
  void processMessageAndSchemaMustShareTheSameContext() throws Exception {
    String body =
        "{\"count\":1,\"offset\":0,\"totalCount\":1,\"destinations\":[{\"destinationId\":\"fixture\",\"processMessages\":[{\"processModelId\":\"bpmn:fixture\",\"messageId\":\"M1\",\"regions\":[\"DE09\"],\"schemas\":[{\"schemaUri\":\"urn:test:a\",\"mimeType\":\"application/json\"}]},{\"processModelId\":\"bpmn:fixture\",\"messageId\":\"M2\",\"regions\":[\"DE09\"],\"schemas\":[{\"schemaUri\":\"urn:test:b\",\"mimeType\":\"application/json\"}]}]}]}";
    JsonObject input =
        JsonParser.parseString(
                "{\"processModelId\":\"bpmn:fixture\",\"messageId\":\"M1\",\"region\":\"DE091620000000\",\"schema\":\"urn:test:b\"}")
            .getAsJsonObject();
    Report mismatch = new Report("process", "TEST", "v3");
    new Api(mismatch, url -> response(200, body)).process(input);
    assertEquals("error", mismatch.findings.getLast().status());
    input.addProperty("schema", "urn:test:a");
    Report match = new Report("process", "TEST", "v3");
    new Api(match, url -> response(200, body)).process(input);
    assertEquals("confirmed", match.findings.getLast().status());
  }

  @Test
  void invalidPaginationCannotProveAnEmptyResult() throws Exception {
    for (String total : List.of("-1", "4294967296", "0.5", "\"0\"")) {
      Report report = new Report("routing", "TEST", "v3");
      Api api =
          new Api(
              report,
              url ->
                  response(
                      200,
                      "{\"count\":0,\"offset\":0,\"totalCount\":" + total + ",\"routes\":[]}"));
      assertTrue(api.pages("https://example.org/routes", "routes").isEmpty());
      assertEquals("unknown", report.findings.getLast().status());
    }
  }
}
