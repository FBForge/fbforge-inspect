package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import org.junit.jupiter.api.Test;

class ReportContractTest {
  @Test
  void destinationFindingsCarryStableCodesAndKeepTheExistingProtocol() {
    Report report = new Report("destination", "TEST", "v3");
    JsonObject input = JsonParser.parseString("{\"destinationId\":\"expected\"}").getAsJsonObject();
    JsonObject destination =
        JsonParser.parseString(
                "{\"destinationId\":\"other\",\"status\":\"inactive\",\"type\":\"A\",\"metadataVersions\":[\"1.2.0\"]}")
            .getAsJsonObject();
    Api.inspectDestination(destination, input, report, "fixture");
    JsonObject encoded = Main.JSON.toJsonTree(report).getAsJsonObject();
    assertEquals(1, encoded.get("protocolVersion").getAsInt());
    for (FindingCode code :
        new FindingCode[] {FindingCode.DESTINATION_ID, FindingCode.DESTINATION_STATUS}) {
      JsonObject finding =
          encoded.getAsJsonArray("findings").asList().stream()
              .map(JsonElement::getAsJsonObject)
              .filter(item -> item.get("code").getAsString().equals(code.name()))
              .findFirst()
              .orElseThrow();
      assertEquals("error", finding.get("status").getAsString());
      assertFalse(finding.get("title").getAsString().isBlank());
    }
  }

  @Test
  void schemaReferenceGuidanceHasAnExplicitCode() throws Exception {
    Report report = new Report("schema", "TEST", "v3");
    Schemas.inspect("{\"$ref\":\"https://example.org/not-fetched\"}", report);
    assertTrue(
        report.findings.stream()
            .anyMatch(finding -> finding.code() == FindingCode.SCHEMA_REFERENCES_UNRESOLVED));
    assertTrue(report.requests.isEmpty());
  }
}
