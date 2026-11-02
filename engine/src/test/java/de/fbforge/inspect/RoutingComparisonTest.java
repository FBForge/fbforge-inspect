package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import org.junit.jupiter.api.Test;

class RoutingComparisonTest {
  private static JsonObject object(String json) {
    return JsonParser.parseString(json).getAsJsonObject();
  }

  private static JsonObject signature(String context, String destination, boolean qualified) {
    JsonObject result = new JsonObject();
    result.addProperty("context", context);
    result.addProperty("verified", true);
    result.addProperty("contextMatches", qualified);
    JsonObject claims = new JsonObject();
    claims.addProperty("destinationId", destination);
    result.add("claims", claims);
    return result;
  }

  @Test
  void comparesOnlyQualifiedIdentitiesWithoutCollapsingRoutingOrPvogContexts() {
    Report report = new Report("routing", "TEST", "v3");
    JsonArray routes = new JsonArray();
    routes.add(object("{\"destinationId\":\"same\",\"oeId\":\"office-a\"}"));
    routes.add(object("{\"destinationId\":\"same\",\"oeId\":\"office-b\"}"));
    report.data.add("routes", routes);
    report.data.addProperty("pvogRelationsComplete", true);
    JsonArray signatures = new JsonArray();
    report.data.add("addressingSignatures", signatures);
    signatures.add(signature("$.routes[0]", "same", true));
    signatures.add(signature("$.routes[1]", "same", true));
    signatures.add(signature("$.pvogRelations[0].zustaendigkeit[0].idSekundaer[0]", "same", true));
    signatures.add(signature("$.pvogRelations[1].zustaendigkeit[0].idSekundaer[0]", "same", true));
    signatures.add(
        signature("$.pvogRelations[2].zustaendigkeit[0].idSekundaer[0]", "pvog-only", true));
    signatures.add(
        signature("$.pvogRelations[3].zustaendigkeit[0].idSekundaer[0]", "unqualified", false));
    RoutingComparison.inspect(report);
    JsonArray rows = report.data.getAsJsonArray("routingComparison");
    assertEquals(3, rows.size());
    for (int i = 0; i < 2; i++) {
      var row = rows.get(i).getAsJsonObject();
      assertEquals("confirmed", row.get("status").getAsString());
      assertEquals(2, row.getAsJsonArray("pvogContexts").size());
      assertEquals("$.routes[" + i + "]", row.get("routingPath").getAsString());
    }
    assertEquals("warning", rows.get(2).getAsJsonObject().get("status").getAsString());
  }

  @Test
  void incompleteSourcesAndUnqualifiedRoutingNeverBecomeIdentityConfirmation() {
    for (boolean complete : new boolean[] {false, true}) {
      Report report = new Report("routing", "TEST", "v3");
      report.data.add(
          "routes",
          JsonParser.parseString("[{\"destinationId\":\"one\"},{\"destinationId\":\"two\"}]"));
      report.data.addProperty("pvogRelationsComplete", complete);
      JsonArray signatures = new JsonArray();
      signatures.add(signature("$.routes[0]", "one", true));
      signatures.add(signature("$.routes[1]", "two", false));
      report.data.add("addressingSignatures", signatures);
      RoutingComparison.inspect(report);
      var rows = report.data.getAsJsonArray("routingComparison");
      assertEquals(
          complete ? "warning" : "unknown",
          rows.get(0).getAsJsonObject().get("status").getAsString());
      assertEquals("unknown", rows.get(1).getAsJsonObject().get("status").getAsString());
    }
  }
}
