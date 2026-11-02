package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.util.*;
import org.junit.jupiter.api.Test;

class PvogDiscoveryTest {
  private static JsonObject input() {
    return JsonParser.parseString(
            """
            {"regionKind":"ars","region":"020000000000","lbId":"","service":"99008001014002"}
            """)
        .getAsJsonObject();
  }

  private static String service(String id, String leika) {
    return "{\"id\":\""
        + id
        + "\",\"name\":\"Fiktive Leistung\",\"leikakeys\":[\""
        + leika
        + "\"],\"ars\":[\"020000000000\"],\"organisationIds\":[\"L1.OE.office\"]}";
  }

  private static String page(String content, int number, int total, int pages) {
    return "{\"content\":["
        + content
        + "],\"pageable\":{\"pageNumber\":"
        + number
        + ",\"pageSize\":10},\"totalElements\":"
        + total
        + ",\"totalPages\":"
        + pages
        + "}";
  }

  @Test
  void automaticallyInspectsTheOnlyCompleteCandidateAndRetainsItsSource() throws Exception {
    Report report = new Report("routing", "TEST", "v3");
    List<String> requests = new ArrayList<>();
    Api api =
        new Api(
            report,
            url -> {
              requests.add(url);
              if (url.contains("/leikaid?"))
                return ApiTest.response(
                    200, page(service("L1.LB.service", "99008001014002"), 0, 1, 1));
              if (url.contains("/relations/"))
                return ApiTest.response(
                    200,
                    "{\"content\":[],\"page\":{\"number\":0,\"size\":100,\"totalElements\":0,\"totalPages\":0}}");
              return ApiTest.response(
                  200,
                  "{\"id\":{\"schemeAgencyID\":\"L1\",\"value\":\"service\"},\"referenzLeistungsschluessel\":[{\"value\":\"99008001014002\"}]}");
            });
    api.pvog(input(), new Signatures(api));
    assertEquals(3, requests.size());
    assertTrue(
        requests
            .getFirst()
            .startsWith(
                "https://stage.pvog.fitko.net/suchdienst/api/v3/servicedescriptions/leikaid?"));
    assertTrue(requests.getFirst().contains("leikaIds=99008001014002"));
    assertTrue(requests.getFirst().contains("ars=020000000000"));
    assertTrue(requests.getFirst().contains("withOrgIds=true"));
    JsonObject candidate = report.data.getAsJsonArray("pvogCandidates").get(0).getAsJsonObject();
    assertEquals("L1.LB.service", candidate.get("id").getAsString());
    assertEquals("L1.OE.office", candidate.getAsJsonArray("organisationIds").get(0).getAsString());
    assertEquals(requests.getFirst(), candidate.get("source").getAsString());
    assertTrue(report.data.has("pvogRelations"));
    assertTrue(report.data.get("pvogAutomaticallySelected").getAsBoolean());
    assertEquals("L1.LB.service", report.data.get("pvogSelectedLbId").getAsString());
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("PVOG-Leistungszuordnung") && f.status().equals("confirmed")));
  }

  @Test
  void rejectsForeignLeikaAndDeduplicatesIdsWithoutLosingOrganisationReferences() throws Exception {
    Report report = new Report("routing", "PROD", "v2");
    Api api =
        new Api(
            report,
            url -> {
              assertTrue(url.startsWith("https://pvog.fitko.net/"));
              return ApiTest.response(
                  200,
                  page(
                      service("L1.LB.other", "00000000000000")
                          + ","
                          + service("L1.LB.service", "99008001014002")
                          + ","
                          + service("L1.LB.service", "99008001014002")
                              .replace("L1.OE.office", "L1.OE.second"),
                      0,
                      3,
                      1));
            });
    api.pvog(input(), new Signatures(api));
    JsonArray candidates = report.data.getAsJsonArray("pvogCandidates");
    assertEquals(1, candidates.size());
    assertEquals(2, candidates.get(0).getAsJsonObject().getAsJsonArray("organisationIds").size());
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f ->
                    f.title().equals("PVOG-Vorschlag passt nicht zur Leistung")
                        && f.status().equals("error")));
  }

  @Test
  void keepsPartialSuggestionsButDoesNotConfirmIncompleteSearch() throws Exception {
    Report report = new Report("routing", "TEST", "v3");
    Api api =
        new Api(
            report,
            url ->
                ApiTest.response(200, page(service("L1.LB.service", "99008001014002"), 4, 1, 1)));
    api.pvog(input(), new Signatures(api));
    assertEquals(1, report.data.getAsJsonArray("pvogCandidates").size());
    assertFalse(report.data.has("pvogSelectedLbId"));
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f -> f.title().equals("PVOG-Vorschlagssuche") && f.status().equals("unknown")));
  }

  @Test
  void deniedSearchIsNotReportedAsAnEmptySuccessfulResult() throws Exception {
    Report report = new Report("routing", "TEST", "v3");
    Api api = new Api(report, url -> ApiTest.response(403, "{}"));
    api.pvog(input(), new Signatures(api));
    assertTrue(report.data.getAsJsonArray("pvogCandidates").isEmpty());
    assertFalse(
        report.findings.stream()
            .anyMatch(
                f -> f.title().equals("PVOG-Vorschlagssuche") && f.status().equals("confirmed")));
  }

  @Test
  void paginatesWithinTheSuggestionBudgetAndMarksTruncationExplicitly() throws Exception {
    for (int total : new int[] {20, 30}) {
      Report report = new Report("routing", "TEST", "v3");
      List<String> urls = new ArrayList<>();
      Api api =
          new Api(
              report,
              url -> {
                int number = urls.size();
                urls.add(url);
                assertTrue(url.endsWith("page=" + number));
                List<String> rows = new ArrayList<>();
                for (int i = 0; i < 10; i++)
                  rows.add(service("L1.LB.service" + (number * 10 + i), "99008001014002"));
                return ApiTest.response(
                    200, page(String.join(",", rows), number, total, total / 10));
              });
      api.pvog(input(), new Signatures(api));
      assertEquals(2, urls.size());
      assertEquals(20, report.data.getAsJsonArray("pvogCandidates").size());
      assertFalse(report.data.has("pvogSelectedLbId"));
      String expectedStatus = total == 20 ? "confirmed" : "unknown";
      assertTrue(
          report.findings.stream()
              .anyMatch(
                  f ->
                      f.title().equals("PVOG-Vorschlagssuche")
                          && f.status().equals(expectedStatus)));
    }
  }

  @Test
  void oversizedResponseCannotExceedTheAdvertisedPerPageSuggestionLimit() throws Exception {
    List<String> rows = new ArrayList<>();
    for (int i = 0; i < 25; i++) rows.add(service("L1.LB.service" + i, "99008001014002"));
    Report report = new Report("routing", "TEST", "v3");
    Api api = new Api(report, url -> ApiTest.response(200, page(String.join(",", rows), 0, 25, 3)));
    api.pvog(input(), new Signatures(api));
    assertEquals(10, report.data.getAsJsonArray("pvogCandidates").size());
    assertTrue(
        report.findings.stream()
            .anyMatch(
                f -> f.title().equals("PVOG-Vorschlagssuche") && f.status().equals("unknown")));
  }

  @Test
  void explicitServiceIdStillUsesTheExistingComparisonWithoutDiscovery() throws Exception {
    JsonObject request = input();
    request.addProperty("lbId", "L1.LB.service");
    List<String> urls = new ArrayList<>();
    Report report = new Report("routing", "TEST", "v3");
    Api api =
        new Api(
            report,
            url -> {
              urls.add(url);
              return ApiTest.response(404, "{}");
            });
    api.pvog(request, new Signatures(api));
    assertTrue(urls.stream().anyMatch(url -> url.contains("/relations/")));
    assertFalse(urls.stream().anyMatch(url -> url.contains("/leikaid?")));
  }
}
