package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.io.IOException;
import java.util.*;
import org.junit.jupiter.api.Test;

class PvogTest {
  private static final String PARENT = """
      {"uebergeordnetesObjektID":{"value":"od","schemeAgencyID":"L1"},
       "uebergeordnetesObjektKlasse":{"code":"003","listURI":"urn:xoev-de:fim:codeliste:xzufi.transferzustaendigkeittyp"},
       "zustaendigkeit":[{"id":{"value":"relation"},"leistungID":[{"value":"lb","schemeAgencyID":"L1"}],
         "gebietID":[{"value":"010000000000"}],"rolle":[{"code":"01"},{"code":"02"}],
         "kommunikationssystem":[{"kanal":{"code":"004"},"kennung":"unverified-destination"}],
         "gueltigkeit":[{"beginn":"2026-01-01","ende":"2026-12-31"}]}]}
      """;

  private static JsonObject input() {
    return JsonParser.parseString("""
        {"regionKind":"ars","region":"010000000000","lbId":"L1.LB.lb","service":"99150085001000"}
        """).getAsJsonObject();
  }

  private static String page(String content, int number, int size, int total, int pages) {
    return "{\"content\":[" + content + "],\"page\":{\"number\":" + number
        + ",\"size\":" + size + ",\"totalElements\":" + total + ",\"totalPages\":" + pages + "}}";
  }

  private static void inspect(Report report, Api.Fetch fetch) throws IOException {
    Api api = new Api(report, fetch);
    api.pvog(input(), new Signatures(api));
  }

  @Test
  void retainsDuplicateContextsAndScopedParametersWithoutPromotingCommunicationIds() throws Exception {
    Report report = new Report("routing", "TEST", "v3");
    List<String> requests = new ArrayList<>();
    inspect(report, url -> {
      requests.add(url);
      assertTrue(url.startsWith("https://stage.pvog.fitko.net/suchdienst/api/"));
      if (url.contains("/relations/")) return ApiTest.response(200, page(PARENT + "," + PARENT, 0, 100, 2, 1));
      if (url.contains("/servicedescriptions/")) return ApiTest.response(200, """
          {"id":{"value":"lb","schemeAgencyID":"L1"},"referenzLeistungsschluessel":[{"value":"99150085001000"}]}
          """);
      return ApiTest.response(200, """
          {"id":{"value":"od","schemeAgencyID":"L1"},
           "parameterEfA":[{"parameterEfA":{"code":"efa.contact.url"},"parameterWert":"https://example.org","languageCode":["de","en"]}],
           "parameterIndividuell":[{"parameterName":"custom"}],"unrecognisedField":"retained"}
          """);
    });
    assertEquals(2, report.data.getAsJsonArray("pvogRelations").size());
    JsonArray services = report.data.getAsJsonArray("pvogOnlineServices");
    assertEquals(2, services.size());
    assertEquals("$.pvogRelations[0]", services.get(0).getAsJsonObject().get("context").getAsString());
    assertEquals("$.pvogRelations[1]", services.get(1).getAsJsonObject().get("context").getAsString());
    assertEquals(1, report.data.getAsJsonArray("pvogOnlineServiceSources").size());
    assertEquals(0, services.get(0).getAsJsonObject().get("sourceIndex").getAsInt());
    assertEquals(0, services.get(1).getAsJsonObject().get("sourceIndex").getAsInt());
    assertEquals("retained", report.data.getAsJsonArray("pvogOnlineServiceSources").get(0).getAsJsonObject().getAsJsonObject("data").get("unrecognisedField").getAsString());
    assertEquals(3, requests.size()); // Fetch caching does not collapse source contexts.
    assertFalse(report.data.has("addressingSignatures"));
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("PVOG-Leistungszuordnung") && f.status().equals("confirmed")));
  }

  @Test
  void incompleteOrInconsistentPagesNeverProveACompleteNegative() throws Exception {
    for (String response : List.of(
        "{\"content\":[]}",
        page("", 0, 100, 1, 1),
        page("", 0, 100, 0, 1),
        page("null", 0, 100, 1, 1),
        page("{}", 0, 100, 1, 1),
        page(PARENT, 1, 100, 1, 1),
        page(PARENT, 0, 100, 1, 1).replace("\"totalElements\":1", "\"totalElements\":\"1\""))) {
      Report report = new Report("routing", "PROD", "v3");
      inspect(report, url -> ApiTest.response(url.contains("/relations/") ? 200 : 404, url.contains("/relations/") ? response : "{}"));
      assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("PVOG-Quellenumfang") && f.status().equals("unknown")), response);
    }
  }

  @Test
  void validEmptyPageIsCompleteButDoesNotInventParameters() throws Exception {
    Report report = new Report("routing", "PROD", "v3");
    inspect(report, url -> ApiTest.response(url.contains("/relations/") ? 200 : 404, page("", 0, 100, 0, 0)));
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("PVOG-Quellenumfang") && f.status().equals("confirmed")));
    assertTrue(report.data.getAsJsonArray("pvogOnlineServices").isEmpty());
  }

  @Test
  void laterFailurePreservesContextsAndReportsPartialScope() throws Exception {
    Report report = new Report("routing", "TEST", "v3");
    inspect(report, url -> {
      if (url.contains("page=0")) return ApiTest.response(200, page(PARENT, 0, 1, 2, 2));
      throw new IOException("synthetic network failure");
    });
    assertEquals(1, report.data.getAsJsonArray("pvogRelations").size());
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("PVOG-Quellenumfang") && f.status().equals("unknown")));
    assertFalse(report.findings.stream().anyMatch(f -> f.title().equals("PVOG-Onlinedienstzuordnung") && f.status().equals("confirmed")));
  }

  @Test
  void foreignOnlineServiceIdIsNotAssignedToTheParent() throws Exception {
    Report report = new Report("routing", "STAGE", "v3");
    inspect(report, url -> {
      assertTrue(url.startsWith("https://pvog.fitko.net/"));
      return ApiTest.response(200, url.contains("/relations/") ? page(PARENT, 0, 100, 1, 1)
          : "{\"id\":{\"value\":\"foreign\",\"schemeAgencyID\":\"L1\"},\"parameterEfA\":[]}");
    });
    assertFalse(report.data.getAsJsonArray("pvogOnlineServices").get(0).getAsJsonObject().get("matchesParent").getAsBoolean());
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("PVOG-Onlinedienstzuordnung") && f.status().equals("error")));
  }

  @Test
  void missingOrForeignObjectClassDoesNotTriggerAnOnlineServiceLookup() throws Exception {
    for (String parent : List.of(PARENT.replace("\"003\"", "\"004\""), PARENT.replace(Pvog.TRANSFER_TYPE, "urn:foreign"))) {
      Report report = new Report("routing", "TEST", "v3");
      inspect(report, url -> {
        assertFalse(url.contains("/onlineservices/"));
        return ApiTest.response(url.contains("/relations/") ? 200 : 404, page(parent, 0, 100, 1, 1));
      });
      assertTrue(report.data.getAsJsonArray("pvogOnlineServices").isEmpty());
    }
  }

  @Test
  void malformedIdentifiersAndForeignServiceResponsesRetainTheSourceWithoutConfirmation() throws Exception {
    Report report = new Report("routing", "PROD", "v3");
    inspect(report, url -> ApiTest.response(200, url.contains("/relations/")
        ? page(PARENT.replace("\"schemeAgencyID\":\"L1\"", "\"schemeAgencyID\":{}"), 0, 100, 1, 1)
        : "{\"id\":{\"value\":\"foreign\",\"schemeAgencyID\":\"L1\"},\"referenzLeistungsschluessel\":[{\"value\":\"99150085001000\"}]}"));
    assertEquals(1, report.data.getAsJsonArray("pvogRelations").size());
    assertFalse(report.findings.stream().anyMatch(f -> f.title().equals("PVOG-Leistungszuordnung") && f.status().equals("confirmed")));
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("PVOG-Leistungsbeschreibungszuordnung") && f.status().equals("error")));
  }
}
