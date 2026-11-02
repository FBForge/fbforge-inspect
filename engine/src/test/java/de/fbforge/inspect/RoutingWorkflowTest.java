package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.util.*;
import org.junit.jupiter.api.*;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

class RoutingWorkflowTest {
  private static JsonObject input(String kind, String region) {
    JsonObject input = new JsonObject();
    input.addProperty("service", SignatureFixtures.SERVICE);
    input.addProperty("regionKind", kind);
    input.addProperty("region", region);
    return input;
  }

  private static String page(String field, String content) {
    return "{\"" + field + "\":" + content + ",\"totalCount\":0,\"offset\":0,\"count\":0}";
  }

  @ParameterizedTest
  @CsvSource({
    "TEST,ars,DE020000000000,ars=020000000000",
    "STAGE,ags,02000000,ags=02000000",
    "PROD,areaId,12345,areaId=12345"
  })
  void bindsRegionKindAndEnvironmentWithoutGuessing(
      String env, String kind, String region, String query) throws Exception {
    Report report = new Report("routing", env, "v3");
    List<String> calls = new ArrayList<>();
    Api api =
        new Api(
            report,
            url -> {
              calls.add(url);
              return SignatureFixtures.response(url, page("routes", "[]"));
            });
    api.routes(input(kind, region));
    assertEquals(
        List.of(
            Api.routing(env)
                + "/v2/routes?leikaKey="
                + SignatureFixtures.SERVICE
                + "&"
                + query
                + "&limit=100&offset=0"),
        calls);
    assertTrue(report.data.getAsJsonArray("routes").isEmpty());
    assertEquals(
        env.equals("STAGE"),
        report.findings.stream().anyMatch(f -> f.title().equals("Routingquelle PROD")));
  }

  @ParameterizedTest
  @CsvSource({"ars,0200", "ags,020000000000", "areaId,abc", "unknown,02"})
  void invalidRegionsNeverFetch(String kind, String region) {
    Report report = new Report("routing", "TEST", "v3");
    Api api =
        new Api(
            report,
            url -> {
              fail("no request for invalid region");
              return null;
            });
    assertThrows(IllegalArgumentException.class, () -> api.routes(input(kind, region)));
  }

  @Test
  void keepsRepeatedDestinationsAndTheirDistinctSignaturePaths() throws Exception {
    Report report = new Report("routing", "TEST", "v3");
    JsonArray rows = new JsonArray();
    JsonObject row = new JsonObject();
    row.addProperty("destinationId", SignatureFixtures.DESTINATION);
    row.addProperty(
        "destinationSignature", SignatureFixtures.token(SignatureFixtures.claims("TEST").build()));
    rows.add(row);
    rows.add(row.deepCopy());
    JsonObject response = new JsonObject();
    response.add("routes", rows);
    response.addProperty("totalCount", 2);
    response.addProperty("count", 2);
    response.addProperty("offset", 0);
    Api api =
        new Api(
            report,
            url ->
                SignatureFixtures.response(
                    url, url.contains("jwks") ? SignatureFixtures.jwks() : response.toString()));
    api.routes(input("ars", SignatureFixtures.ARS));
    assertEquals(2, report.data.getAsJsonArray("routes").size());
    JsonArray signatures = report.data.getAsJsonArray("addressingSignatures");
    assertEquals("$.routes[0]", signatures.get(0).getAsJsonObject().get("context").getAsString());
    assertEquals("$.routes[1]", signatures.get(1).getAsJsonObject().get("context").getAsString());
  }

  @Test
  void missingSignaturesRemainUnknownAndPvogDiscoveryIsExplicit() throws Exception {
    Report report = new Report("routing", "TEST", "v3");
    List<String> calls = new ArrayList<>();
    Api api =
        new Api(
            report,
            url -> {
              calls.add(url);
              return SignatureFixtures.response(
                  url,
                  url.contains("leikaid")
                      ? "{\"content\":[],\"pageable\":{\"pageNumber\":0,\"pageSize\":10},\"totalElements\":0,\"totalPages\":0}"
                      : "{\"routes\":[{\"destinationId\":\"fixture\"}],\"totalCount\":1,\"count\":1,\"offset\":0}");
            });
    JsonObject input = input("ars", SignatureFixtures.ARS);
    input.addProperty("pvog", true);
    api.routes(input);
    assertEquals(2, calls.size());
    assertTrue(calls.get(1).startsWith("https://stage.pvog.fitko.net/"));
    assertTrue(report.data.has("pvogCandidates"));
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("Adressierungssignatur fehlt")
                        && f.status().equals("unknown")));
  }

  @Test
  void areaSearchEscapesNamesAndBoundsInput() throws Exception {
    Report report = new Report("areas", "TEST", "v3");
    List<String> calls = new ArrayList<>();
    Api api =
        new Api(
            report,
            url -> {
              calls.add(url);
              return SignatureFixtures.response(url, page("areas", "[]"));
            });
    JsonObject input = new JsonObject();
    input.addProperty("areaSearch", "Bad A&B");
    api.areas(input);
    assertTrue(calls.getFirst().contains("areaSearchexpression=Bad+A%26B"));
    for (String name : List.of("a", "x".repeat(151))) {
      input.addProperty("areaSearch", name);
      assertThrows(IllegalArgumentException.class, () -> api.areas(input));
    }
    assertEquals(1, calls.size());
  }
}
