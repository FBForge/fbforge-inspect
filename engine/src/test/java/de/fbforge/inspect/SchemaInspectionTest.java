package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.*;
import org.junit.jupiter.api.Test;

class SchemaInspectionTest {
  private static Report report() {
    return new Report("schema", "TEST", "v3");
  }

  @Test
  void malformedJsonAndNonSchemaRootsAreRejected() {
    for (String input : new String[] {"{\"type\":", "[]", "42", "null", "\"object\"",
        "{'type':'object'}", "/*comment*/{\"type\":\"object\"}", "{\"type\":object}", "{\"enum\":[1,]}"})
      assertThrows(Exception.class, () -> Schemas.inspect(input, report()), input);
  }

  @Test
  void booleanSchemasAreAcceptedWithoutInventingStructure() throws Exception {
    for (String input : new String[] {"true", "false"}) {
      Report report = report();
      Schemas.inspect(input, report);
      assertEquals("Boolean JSON Schema", report.data.get("dialect").getAsString());
      assertEquals(0, report.data.getAsJsonArray("structure").size());
    }
  }

  @Test
  void utf8BomDoesNotMisclassifyAnXsdAsJson() throws Exception {
    Report report = report();
    Schemas.inspect("\uFEFF<xs:schema xmlns:xs='http://www.w3.org/2001/XMLSchema' version='2.3.1' targetNamespace='urn:fixture'/>", report);
    assertEquals("XSD", report.data.get("dialect").getAsString());
    assertEquals("2.3.1", report.data.get("schemaVersion").getAsString());
  }

  @Test
  void jsonStructuralNodeBudgetIsExactly4000() throws Exception {
    JsonObject schema = new JsonObject();
    for (int i = 0; i < 3999; i++) schema.add("field" + i, new JsonObject());
    Report report = report();
    Schemas.inspect(schema.toString(), report);
    assertEquals(4000, report.data.getAsJsonArray("structure").size());
    schema.add("oneTooMany", new JsonObject());
    assertThrows(IllegalArgumentException.class, () -> Schemas.inspect(schema.toString(), report()));
  }

  @Test
  void xsdStructuralNodeBudgetIsExactly4000() throws Exception {
    String prefix = "<xs:schema xmlns:xs='http://www.w3.org/2001/XMLSchema'>";
    String nodes = "<xs:element name='fixture' type='xs:string'/>".repeat(3999);
    Report report = report();
    Schemas.inspect(prefix + nodes + "</xs:schema>", report);
    assertEquals(4000, report.data.getAsJsonArray("structure").size());
    assertThrows(IllegalArgumentException.class,
        () -> Schemas.inspect(prefix + nodes + "<xs:element name='excess'/></xs:schema>", report()));
  }

  @Test
  void importsAndIncludesRemainUnresolvedEvidence() throws Exception {
    Report report = report();
    Schemas.inspect("""
        <xs:schema xmlns:xs='http://www.w3.org/2001/XMLSchema'>
          <xs:import namespace='urn:foreign' schemaLocation='https://example.org/foreign.xsd'/>
          <xs:include schemaLocation='relative.xsd'/>
        </xs:schema>
        """, report);
    assertEquals(2, report.data.getAsJsonArray("references").size());
    assertTrue(report.requests.isEmpty());
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("Referenzen nicht aufgelöst") && f.status().equals("unknown")));
  }

  @Test
  void deepSchemasAreRejectedAndParsingIsNeverPresentedAsFullValidation() throws Exception {
    String deep = "{\"properties\":{\"child\":".repeat(55) + "{}" + "}}".repeat(55);
    assertThrows(IllegalArgumentException.class, () -> Schemas.inspect(deep, report()));
    Report report = report();
    // Syntactically JSON, but intentionally not a valid JSON Schema. This inspector does not
    // implement metaschema or instance validation and must keep that limitation explicit.
    Schemas.inspect("{\"type\":\"not-a-json-schema-type\",\"minimum\":\"invalid\"}", report);
    assertTrue(report.findings.stream().anyMatch(f -> f.title().equals("Schema eingelesen")
        && f.limitation().contains("keine vollständige Schema- oder Instanzvalidierung")));
  }
}
