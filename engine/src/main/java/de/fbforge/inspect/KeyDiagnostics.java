package de.fbforge.inspect;

import com.google.gson.*;
import com.nimbusds.jose.jwk.*;
import java.util.*;
import java.util.regex.Pattern;

public final class KeyDiagnostics {
  static final String SOURCE = "https://docs.fitko.de/fit-connect/docs/details/crypto/";
  static final Set<String> PRIVATE = Set.of("d", "p", "q", "dp", "dq", "qi", "oth", "k");

  public static boolean hasPrivateMaterial(String text) {
    return hasPrivateMaterial(text, false);
  }

  public static boolean hasPrivateMaterial(String text, boolean strictKeyInput) {
    if (text.contains("PRIVATE KEY")) return true;
    try {
      return privateFields(JsonParser.parseString(text), strictKeyInput);
    } catch (Exception e) {
      return Pattern.compile("\"(?:d|p|q|dp|dq|qi|oth|k)\"\\s*:").matcher(text).find();
    }
  }

  private static boolean privateFields(JsonElement e, boolean strictKeyInput) {
    if (e.isJsonArray()) {
      for (JsonElement child : e.getAsJsonArray())
        if (privateFields(child, strictKeyInput)) return true;
    }
    if (e.isJsonObject())
      for (var entry : e.getAsJsonObject().entrySet()) {
        if ((strictKeyInput
                    || e.getAsJsonObject().has("kty")
                        && e.getAsJsonObject().get("kty").isJsonPrimitive()
                        && e.getAsJsonObject().get("kty").getAsJsonPrimitive().isString())
                && PRIVATE.contains(entry.getKey())
            || entry.getKey().equals("kty")
                && entry.getValue().isJsonPrimitive()
                && entry.getValue().getAsString().equals("oct")
            || privateFields(entry.getValue(), strictKeyInput)) return true;
      }
    return false;
  }

}
