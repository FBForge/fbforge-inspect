package de.fbforge.inspect;

import com.nimbusds.jwt.SignedJWT;
import java.util.Set;

/** Avoid retaining claims from unrelated credential tokens in the standalone public workbench. */
final class PublicSignatureInput {
  static void validate(String text) {
    try {
      var claims = SignedJWT.parse(text).getJWTClaimsSet();
      JsonFields.uuid(claims.getStringClaim("destinationId"));
      if (!Set.of(
                  "test.fit-connect.fitko.dev",
                  "prod.fit-connect.fitko.net",
                  "stage.fit-connect.fitko.net")
              .contains(claims.getStringClaim("submissionHost"))
          || !(claims.getClaim("services") instanceof java.util.List<?>))
        throw new IllegalArgumentException();
    } catch (Exception invalid) {
      throw new IllegalArgumentException(
          "Öffentliche destinationSignature mit Zustellpunkt und Leistungskontext erwartet. Keine"
              + " Zugangstoken verwenden.");
    }
  }
}
