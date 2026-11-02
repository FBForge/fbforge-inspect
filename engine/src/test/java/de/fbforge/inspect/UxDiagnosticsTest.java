package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import java.util.ArrayList;
import org.junit.jupiter.api.Test;

class UxDiagnosticsTest {
  @Test
  void limitsDoNotRequireOrRequestAProcessSearch() throws Exception {
    Report report = new Report("limits", "TEST", "v3");
    var urls = new ArrayList<String>();
    Api api =
        new Api(
            report,
            url -> {
              urls.add(url);
              return ApiTest.response(200, "{\"attachments\":{\"applicable\":true}}");
            });
    api.limits(
        JsonParser.parseString(
                "{\"fromDestinationId\":\"00000000-0000-4000-8000-000000000001\",\"toDestinationId\":\"00000000-0000-4000-8000-000000000002\"}")
            .getAsJsonObject());
    assertEquals(1, urls.size());
    assertTrue(
        urls.getFirst()
            .endsWith(
                "/v3/limits/00000000-0000-4000-8000-000000000001/00000000-0000-4000-8000-000000000002"));
    assertTrue(report.data.has("limits"));
    assertEquals("confirmed", report.findings.getFirst().status());
  }

  @Test
  void accessRejectionRemainsUnknownWithAnActionableExplanation() throws Exception {
    Report report = new Report("process", "TEST", "v3");
    assertNull(
        new Api(report, url -> ApiTest.response(401, "{}")).json("https://example.org/public"));
    assertEquals("unknown", report.findings.getFirst().status());
    assertTrue(report.findings.getFirst().detail().contains("Zugriff zurückgewiesen"));
    assertTrue(report.findings.getFirst().nextStep().contains("kein leerer Suchbestand"));
  }

  @Test
  void syntaxErrorsIdentifyFormatAndPositionWithoutReturningInput() {
    var invalid =
        assertThrows(
            IllegalArgumentException.class,
            () -> Schemas.inspect("{\"private-marker\":", new Report("schema", "TEST", "v3")));
    String safe = Main.safeMessage(invalid);
    assertTrue(safe.startsWith("JSON-Syntaxfehler"));
    assertTrue(safe.contains("Zeile"));
    assertFalse(safe.contains("private-marker"));
    var xml =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                Schemas.inspect(
                    "<xs:schema xmlns:xs='http://www.w3.org/2001/XMLSchema'><broken></xs:schema>",
                    new Report("schema", "TEST", "v3")));
    assertTrue(Main.safeMessage(xml).startsWith("XML-Syntaxfehler in Zeile"));
    assertFalse(Main.safeMessage(xml).contains("broken"));
  }
}
