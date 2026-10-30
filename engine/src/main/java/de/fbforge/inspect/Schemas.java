package de.fbforge.inspect;

import com.google.gson.*;
import java.io.*;
import java.util.*;
import javax.xml.XMLConstants;
import javax.xml.parsers.*;
import org.w3c.dom.*;
import org.xml.sax.InputSource;
import org.xml.sax.helpers.DefaultHandler;

/** Structural inspection only. References are enumerated, never fetched implicitly. */
final class Schemas {
  private static final Gson SCHEMA_JSON = new GsonBuilder().setStrictness(Strictness.STRICT).create();

  static void inspect(String text, Report report) throws Exception {
    if (text.startsWith("\uFEFF")) text = text.substring(1);
    JsonArray nodes = new JsonArray(), refs = new JsonArray();
    if (text.stripLeading().startsWith("<")) {
      DocumentBuilderFactory f = DocumentBuilderFactory.newInstance();
      f.setNamespaceAware(true);
      f.setFeature(XMLConstants.FEATURE_SECURE_PROCESSING, true);
      f.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
      f.setFeature("http://xml.org/sax/features/external-general-entities", false);
      f.setFeature("http://xml.org/sax/features/external-parameter-entities", false);
      f.setAttribute(XMLConstants.ACCESS_EXTERNAL_DTD, "");
      f.setAttribute(XMLConstants.ACCESS_EXTERNAL_SCHEMA, "");
      f.setXIncludeAware(false);
      f.setExpandEntityReferences(false);
      var builder = f.newDocumentBuilder();
      builder.setErrorHandler(new DefaultHandler());
      Document doc = builder.parse(new InputSource(new StringReader(text)));
      Element root = doc.getDocumentElement();
      if (!XMLConstants.W3C_XML_SCHEMA_NS_URI.equals(root.getNamespaceURI())
          || !"schema".equals(root.getLocalName()))
        throw new IllegalArgumentException("XSD-Schemawurzel erwartet.");
      xml(root, "$", nodes, refs, 0);
      // xs:schema/@version identifies the schema document, not the XSD language version.
      report.data.addProperty("dialect", "XSD");
      report.data.addProperty("schemaVersion", root.getAttribute("version"));
      report.data.addProperty("targetNamespace", root.getAttribute("targetNamespace"));
    } else {
      JsonElement root = SCHEMA_JSON.fromJson(text, JsonElement.class);
      if (!root.isJsonObject() && !root.isJsonPrimitive())
        throw new IllegalArgumentException("JSON-Schemawurzel erwartet.");
      if (root.isJsonPrimitive() && !root.getAsJsonPrimitive().isBoolean())
        throw new IllegalArgumentException(
            "JSON-Schema muss ein Objekt oder boolescher Wert sein.");
      report.data.addProperty(
          "dialect",
          root.isJsonObject()
              ? Api.value(root.getAsJsonObject(), "$schema")
              : "Boolean JSON Schema");
      json(root, "$", nodes, refs, 0);
    }
    report.data.add("structure", nodes);
    report.data.add("references", refs);
    report.add(
        "confirmed",
        "Schema eingelesen",
        nodes.size() + " Strukturknoten dargestellt.",
        "$",
        "",
        "",
        "Syntaxprüfung und Strukturansicht, keine vollständige Schema- oder Instanzvalidierung.");
    if (!refs.isEmpty())
      report.unknown(
          "Referenzen nicht aufgelöst",
          refs.size()
              + " Referenzen gefunden. Fremdressourcen werden nur einzeln auf ausdrücklichen Aufruf"
              + " geöffnet.",
          "$",
          "");
  }

  private static void json(JsonElement e, String path, JsonArray nodes, JsonArray refs, int depth) {
    if (depth > 48)
      throw new IllegalArgumentException("Strukturlimit erreicht.");
    if (e.isJsonObject()) {
      if (nodes.size() >= 4000) throw new IllegalArgumentException("Strukturlimit erreicht.");
      JsonObject o = e.getAsJsonObject(), node = new JsonObject();
      node.addProperty("path", path);
      for (String p : List.of("title", "description", "type", "required", "enum"))
        if (o.has(p)) node.add(p, o.get(p));
      nodes.add(node);
      for (String p : List.of("$ref", "$dynamicRef", "$recursiveRef"))
        if (o.has(p)) {
          JsonObject ref = new JsonObject();
          ref.addProperty("path", path + "." + p);
          ref.add("target", o.get(p));
          refs.add(ref);
        }
      for (var entry : o.entrySet())
        if (entry.getValue().isJsonObject() || entry.getValue().isJsonArray())
          json(entry.getValue(), path + "." + entry.getKey(), nodes, refs, depth + 1);
    } else if (e.isJsonArray()) {
      int i = 0;
      for (JsonElement child : e.getAsJsonArray())
        json(child, path + "[" + (i++) + "]", nodes, refs, depth + 1);
    }
  }

  private static void xml(Element e, String path, JsonArray nodes, JsonArray refs, int depth) {
    // Documentation and appinfo are not structural schema nodes. Official XZuFi packages
    // contain extensive annotations, including foreign XML, that must not exhaust this budget.
    if (XMLConstants.W3C_XML_SCHEMA_NS_URI.equals(e.getNamespaceURI())
        && "annotation".equals(e.getLocalName())) return;
    if (depth > 48 || nodes.size() >= 4000)
      throw new IllegalArgumentException("Strukturlimit erreicht.");
    String p =
        path
            + "/"
            + e.getLocalName()
            + (e.hasAttribute("name") ? "[@name='" + e.getAttribute("name") + "']" : "");
    JsonObject node = new JsonObject();
    node.addProperty("path", p);
    for (String a : List.of("name", "type", "ref", "minOccurs", "maxOccurs", "use", "base"))
      if (e.hasAttribute(a)) node.addProperty(a, e.getAttribute(a));
    nodes.add(node);
    if (e.hasAttribute("schemaLocation")) {
      JsonObject ref = new JsonObject();
      ref.addProperty("path", p);
      ref.addProperty("target", e.getAttribute("schemaLocation"));
      refs.add(ref);
    }
    NodeList children = e.getChildNodes();
    for (int i = 0; i < children.getLength(); i++)
      if (children.item(i) instanceof Element child) xml(child, p, nodes, refs, depth + 1);
  }
}
