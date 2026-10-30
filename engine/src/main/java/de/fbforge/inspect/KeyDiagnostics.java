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

  public static void inspect(String text, Report report, PublicHttp http, JsonObject options) {
    if (hasPrivateMaterial(text, true))
      throw new IllegalArgumentException(
          "Private oder symmetrische Schlüssel sind nicht zulässig. Eingabe wurde verworfen.");
    JsonElement parsed = JsonParser.parseString(text);
    JsonArray keys = new JsonArray();
    if (parsed.isJsonObject() && parsed.getAsJsonObject().has("keys"))
      keys = parsed.getAsJsonObject().getAsJsonArray("keys");
    else keys.add(parsed);
    if (keys.isEmpty() || keys.size() > 32)
      throw new IllegalArgumentException("Zwischen 1 und 32 öffentlichen Schlüsseln erwartet.");
    Set<String> kids = new HashSet<>();
    JsonArray summary = new JsonArray();
    for (int i = 0; i < keys.size(); i++) {
      String path = "$.keys[" + i + "]";
      try {
        JWK key = JWK.parse(keys.get(i).toString());
        boolean rsa = key instanceof RSAKey;
        report.check(rsa, "Schlüsseltyp", key.getKeyType().toString(), path + ".kty", SOURCE);
        boolean unique =
            key.getKeyID() != null && !key.getKeyID().isBlank() && kids.add(key.getKeyID());
        report.check(
            unique,
            "Schlüsselkennung",
            unique ? "kid ist vorhanden und eindeutig." : "kid fehlt oder ist doppelt.",
            path + ".kid",
            SOURCE);
        if (rsa) {
          report.check(
              key.size() >= 3072,
              "RSA-Schlüssellänge",
              key.size() + " Bit (Mindestvorgabe 3072)",
              path,
              SOURCE);
          report.check(
              ((RSAKey) key)
                  .getPublicExponent()
                  .decodeToBigInteger()
                  .equals(java.math.BigInteger.valueOf(65537)),
              "RSA-Exponent",
              "65537 (AQAB) erwartet.",
              path + ".e",
              SOURCE);
        }
        String alg = key.getAlgorithm() == null ? "" : key.getAlgorithm().getName();
        report.check(
            Set.of("RSA-OAEP-256", "PS512").contains(alg),
            "Algorithmus",
            alg.isEmpty() ? "alg fehlt." : alg,
            path + ".alg",
            SOURCE);
        String operation = alg.equals("PS512") ? "verify" : "wrapKey";
        var ops = key.getKeyOperations();
        boolean allowed =
            ops != null && ops.size() == 1 && ops.iterator().next().identifier().equals(operation);
        report.check(
            allowed,
            "Schlüsseloperation",
            operation + " als einzige key_ops-Operation erwartet.",
            path + ".key_ops",
            SOURCE);
        report.check(
            key.getKeyUse() == null
                || key.getKeyUse().identifier().equals(alg.equals("PS512") ? "sig" : "enc"),
            "Verwendungszweck",
            "use muss zu alg passen, falls angegeben.",
            path + ".use",
            SOURCE);
        JsonObject item = new JsonObject();
        item.addProperty("kid", key.getKeyID());
        item.addProperty("algorithm", alg);
        item.addProperty("bits", key.size());
        item.add("publicJwk", JsonParser.parseString(key.toPublicJWK().toJSONString()));
        item.addProperty(
            "x5cCount", key.getX509CertChain() == null ? 0 : key.getX509CertChain().size());
        summary.add(item);
        Certificates.inspect(key, report, http, options, path);
      } catch (Exception e) {
        report.add(
            "error",
            "Schlüssel nicht auswertbar",
            "Öffentliches JWK-Format oder Zertifikatsdaten ungültig.",
            path,
            SOURCE,
            "JWK und x5c prüfen.",
            "");
      }
    }
    report.data.add("keys", summary);
  }
}
