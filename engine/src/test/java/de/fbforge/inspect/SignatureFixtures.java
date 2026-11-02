package de.fbforge.inspect;

import com.google.gson.*;
import com.nimbusds.jose.*;
import com.nimbusds.jose.crypto.RSASSASigner;
import com.nimbusds.jose.jwk.*;
import com.nimbusds.jose.jwk.gen.RSAKeyGenerator;
import com.nimbusds.jwt.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.*;

/** Generated signing fixtures contain no real service identity or credentials. */
final class SignatureFixtures {
  static final String DESTINATION = "00000000-0000-4000-8000-000000000001",
      SERVICE = "99008001014002",
      ARS = "020000000000";

  private static final class Keys {
    static final RSAKey KEY = create();

    private static RSAKey create() {
      try {
        return new RSAKeyGenerator(4096).keyID("fixture").generate();
      } catch (Exception failure) {
        throw new ExceptionInInitializerError(failure);
      }
    }
  }

  static RSAKey key() {
    return Keys.KEY;
  }

  static JWTClaimsSet.Builder claims(String environment) {
    String host =
        URI.create(Api.submission(environment.equals("TEST") ? "TEST" : "PROD")).getHost();
    return new JWTClaimsSet.Builder()
        .issuer("https://" + host)
        .issueTime(Date.from(Instant.now().minusSeconds(60)))
        .expirationTime(Date.from(Instant.now().plusSeconds(600)))
        .jwtID("synthetic")
        .claim("submissionHost", host)
        .claim("destinationId", DESTINATION)
        .claim(
            "services",
            List.of(
                Map.of(
                    "leistungIDs",
                    List.of("urn:de:fim:leika:leistung:" + SERVICE),
                    "gebietIDs",
                    List.of("urn:de:bund:destatis:bevoelkerungsstatistik:schluessel:rs:" + ARS))));
  }

  static String token(JWTClaimsSet claims) throws Exception {
    return token(claims, JWSAlgorithm.PS512, "fixture");
  }

  static String token(JWTClaimsSet claims, JWSAlgorithm algorithm, String kid) throws Exception {
    SignedJWT token = new SignedJWT(new JWSHeader.Builder(algorithm).keyID(kid).build(), claims);
    token.sign(new RSASSASigner(key()));
    return token.serialize();
  }

  static PublicHttp.Response response(String url, String body) {
    return new PublicHttp.Response(
        200, Map.of(), body.getBytes(StandardCharsets.UTF_8), URI.create(url));
  }

  static String jwks() {
    return new JWKSet(key().toPublicJWK()).toString();
  }
}
