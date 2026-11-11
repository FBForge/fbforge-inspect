package de.fbforge.inspect;

import com.google.gson.*;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;

/** Small JSON and service-context helpers shared by public diagnoses. */
final class JsonFields {
  private JsonFields() {}

  public static String value(JsonObject o, String key) {
    return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : "";
  }

  public static String encode(String value) {
    return URLEncoder.encode(value, StandardCharsets.UTF_8);
  }

  static boolean validArs(String value) {
    return value.matches("(?:[0-9]{2}|[0-9]{3}|[0-9]{5}|[0-9]{9}|[0-9]{12})");
  }

  /** Encode one path segment; form/query encoding represents spaces differently. */
  public static String pathSegment(String value) {
    return encode(value).replace("+", "%20");
  }

  public static String uuid(String value) {
    if (!value.matches(
        "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}"))
      throw new IllegalArgumentException("Destination-ID muss eine UUID sein.");
    return value;
  }

  static boolean contains(JsonArray a, String value) {
    for (JsonElement e : a) if (e.isJsonPrimitive() && e.getAsString().equals(value)) return true;
    return false;
  }

  public static JsonArray array(JsonObject o, String name) {
    return o.has(name) && o.get(name).isJsonArray() ? o.getAsJsonArray(name) : new JsonArray();
  }

  static int counter(JsonObject object, String name) {
    JsonElement value = object.get(name);
    if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber())
      throw new IllegalArgumentException("Ungültiger Seitenzähler.");
    int number = value.getAsBigDecimal().intValueExact();
    if (number < 0) throw new IllegalArgumentException("Negativer Seitenzähler.");
    return number;
  }

  static boolean schemaMatch(JsonArray schemas, String schema, String mime) {
    if (schema.isBlank() && mime.isBlank()) return true;
    for (JsonElement e : schemas) {
      JsonObject s = e.getAsJsonObject();
      if ((schema.isBlank() || schema.equals(value(s, "schemaUri")))
          && (mime.isBlank() || mime.equals(value(s, "mimeType")))) return true;
    }
    return false;
  }

  static boolean regionMatches(JsonArray published, String requested) {
    for (JsonElement region : published)
      if (region.isJsonPrimitive()
          && region.getAsString().matches("DE[0-9]{0,12}")
          && requested.startsWith(region.getAsString())) return true;
    return false;
  }

  public static String text(JsonObject object, String key) {
    JsonElement value = object.get(key);
    return value != null && value.isJsonPrimitive() && value.getAsJsonPrimitive().isString()
        ? value.getAsString()
        : "";
  }

  static int strictCounter(JsonObject object, String key) {
    JsonElement value = object.get(key);
    if (value == null
        || !value.isJsonPrimitive()
        || !value.getAsJsonPrimitive().isNumber()
        || !value.getAsString().matches("[0-9]+")) throw new IllegalArgumentException();
    return Integer.parseInt(value.getAsString());
  }
}
