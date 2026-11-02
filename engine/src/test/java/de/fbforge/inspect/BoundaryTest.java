package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import java.net.*;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;

class BoundaryTest {
  @Test
  void ipcRemainsUtf8WithLegacyStdoutEncoding() throws Exception {
    var child =
        new ProcessBuilder(
                Path.of(System.getProperty("java.home"), "bin", "java").toString(),
                "-Dstdout.encoding=windows-1252",
                "-cp",
                System.getProperty("java.class.path"),
                Main.class.getName())
            .redirectError(ProcessBuilder.Redirect.DISCARD)
            .start();
    try {
      child.getOutputStream().write("{\"protocolVersion\":0}".getBytes(StandardCharsets.UTF_8));
      child.getOutputStream().close();
      assertTrue(child.waitFor(15, TimeUnit.SECONDS));
      assertEquals(0, child.exitValue());
      String response =
          StandardCharsets.UTF_8
              .newDecoder()
              .decode(ByteBuffer.wrap(child.getInputStream().readAllBytes()))
              .toString();
      assertEquals(
          "IPC-Version nicht unterstützt.",
          com.google.gson.JsonParser.parseString(response)
              .getAsJsonObject()
              .get("error")
              .getAsString());
    } finally {
      child.destroyForcibly();
    }
  }

  @Test
  void blocksLocalAndReservedAddresses() throws Exception {
    for (String ip :
        new String[] {
          "127.0.0.1",
          "10.1.2.3",
          "169.254.169.254",
          "100.64.0.1",
          "192.168.1.1",
          "::1",
          "fc00::1",
          "2001:db8::1",
          "0.0.0.0"
        }) assertFalse(PublicHttp.isPublic(InetAddress.getByName(ip)), ip);
    assertTrue(PublicHttp.isPublic(InetAddress.getByName("8.8.8.8")));
    assertTrue(PublicHttp.isPublic(InetAddress.getByName("2606:4700:4700::1111")));
  }

  @Test
  void rejectsCredentialsAndUnsafeSchemes() {
    for (String url :
        new String[] {
          "file:///etc/passwd",
          "http://example.org/schema",
          "https://user:pass@example.org",
          "https://example.org:8080/a",
          "https://example.org/?access_token=synthetic"
        })
      assertThrows(
          IllegalArgumentException.class, () -> PublicHttp.validateUri(URI.create(url), false));
  }

  @Test
  void privateInputsRejectedEvenNested() {
    assertTrue(KeyDiagnostics.hasPrivateMaterial("{\"d\":\"partial-private-fixture\"}", true));
    assertTrue(
        KeyDiagnostics.hasPrivateMaterial("{\"keys\":[{\"kty\":\"RSA\",\"d\":\"secret\"}]}"));
    assertTrue(KeyDiagnostics.hasPrivateMaterial("{\"kty\":\"oct\",\"k\":\"secret\"}"));
    assertTrue(KeyDiagnostics.hasPrivateMaterial("-----BEGIN PRIVATE KEY-----"));
    assertFalse(
        KeyDiagnostics.hasPrivateMaterial("{\"kty\":\"RSA\",\"n\":\"abc\",\"e\":\"AQAB\"}"));
    assertFalse(
        KeyDiagnostics.hasPrivateMaterial(
            "{\"properties\":{\"kty\":{\"type\":\"string\"},\"d\":{\"type\":\"string\"}}}"));
  }
}
