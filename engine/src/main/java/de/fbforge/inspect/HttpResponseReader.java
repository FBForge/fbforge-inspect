package de.fbforge.inspect;

import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Bounded HTTP/1 response framing, independent of DNS, sockets and TLS. */
final class HttpResponseReader {
  private HttpResponseReader() {}

  static PublicHttp.Response read(InputStream input, URI uri, NetworkBudget budget)
      throws IOException {
    String statusLine = line(input, budget);
    if (!statusLine.matches("HTTP/1\\.[01] [2-5][0-9]{2}(?: .*)?"))
      throw new IOException("Ungültige HTTP-Antwort.");
    int status = Integer.parseInt(statusLine.substring(9, 12));
    Map<String, String> headers = new TreeMap<>();
    int size = 0;
    String row;
    while (!(row = line(input, budget)).isEmpty()) {
      size += row.length() + 2;
      if (size > 65536) throw new IOException("HTTP-Header zu groß.");
      Map.Entry<String, String> field = header(row);
      if (headers.putIfAbsent(field.getKey(), field.getValue()) != null
          && Set.of("content-length", "transfer-encoding").contains(field.getKey()))
        throw new IOException("Mehrdeutige HTTP-Länge.");
    }
    if (headers.containsKey("content-length") && headers.containsKey("transfer-encoding"))
      throw new IOException("Mehrdeutige HTTP-Länge.");
    if (!headers.getOrDefault("content-encoding", "identity").equalsIgnoreCase("identity"))
      throw new IOException("Komprimierte Antwort nicht zugelassen.");
    ByteArrayOutputStream body = new ByteArrayOutputStream();
    if (headers.containsKey("transfer-encoding")) {
      if (!headers.get("transfer-encoding").equalsIgnoreCase("chunked"))
        throw new IOException("Unbekannte Übertragungskodierung.");
      while (true) {
        String chunk = line(input, budget).split(";", 2)[0];
        if (!chunk.matches("[0-9a-fA-F]+")) throw new IOException("Ungültige Chunk-Länge.");
        int length;
        try {
          length = Integer.parseInt(chunk, 16);
        } catch (NumberFormatException invalid) {
          throw new IOException("Ungültige Chunk-Länge.");
        }
        if (length == 0) {
          trailers(input, budget);
          break;
        }
        copy(input, body, length, budget);
        if (!line(input, budget).isEmpty()) throw new IOException("Ungültiger Chunk-Abschluss.");
      }
    } else if (headers.containsKey("content-length")) {
      String value = headers.get("content-length");
      if (!value.matches("[0-9]+")) throw new IOException("Ungültige Inhaltslänge.");
      int length;
      try {
        length = Integer.parseInt(value);
      } catch (NumberFormatException invalid) {
        throw new IOException("Ungültige Inhaltslänge.");
      }
      copy(input, body, length, budget);
    } else {
      byte[] buffer = new byte[8192];
      int count;
      while (true) {
        budget.checkTime();
        int maximum =
            (int)
                Math.min(
                    buffer.length,
                    Math.min(PublicHttp.MAX_BODY - body.size(), budget.remaining()) + 1);
        count = input.read(buffer, 0, maximum);
        if (count < 0) break;
        if (count == 0) throw new IOException("Antwort ohne Fortschritt.");
        budget.charge(count);
        if ((long) body.size() + count > PublicHttp.MAX_BODY)
          throw new IOException("Antwortbudget erreicht.");
        body.write(buffer, 0, count);
      }
    }
    return new PublicHttp.Response(status, headers, body.toByteArray(), uri);
  }

  static Map.Entry<String, String> header(String row) throws IOException {
    int colon = row.indexOf(':');
    if (colon <= 0
        || !row.substring(0, colon).matches("[!#$%&'*+.^_`|~0-9A-Za-z-]+")
        || row.substring(colon + 1).chars().anyMatch(c -> c < 32 && c != 9 || c == 127))
      throw new IOException("Ungültiger HTTP-Header.");
    return Map.entry(
        row.substring(0, colon).toLowerCase(Locale.ROOT), row.substring(colon + 1).trim());
  }

  private static void trailers(InputStream input, NetworkBudget budget) throws IOException {
    int bytes = 0;
    String row;
    while (!(row = line(input, budget)).isEmpty()) {
      bytes += row.length() + 2;
      if (bytes > 65536
          || Set.of("content-length", "transfer-encoding").contains(header(row).getKey()))
        throw new IOException("Ungültige HTTP-Trailer.");
    }
  }

  private static void copy(
      InputStream input, ByteArrayOutputStream out, int length, NetworkBudget budget)
      throws IOException {
    budget.requireBodySize((long) out.size() + length);
    if (length > budget.remaining()) throw new IOException("Gesamtantwortbudget erreicht.");
    byte[] buffer = new byte[8192];
    while (length > 0) {
      budget.checkTime();
      int count = input.read(buffer, 0, Math.min(length, buffer.length));
      if (count < 0) throw new EOFException();
      if (count == 0) throw new IOException("Antwort ohne Fortschritt.");
      budget.charge(count);
      out.write(buffer, 0, count);
      length -= count;
    }
  }

  static String line(InputStream input, NetworkBudget budget) throws IOException {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    while (true) {
      budget.checkTime();
      int value = input.read();
      if (value < 0) throw new EOFException();
      if (value == '\r') {
        if (input.read() != '\n') throw new IOException("HTTP-Zeile ohne CRLF.");
        return out.toString(StandardCharsets.ISO_8859_1);
      }
      if (value == '\n' || out.size() >= 8192) throw new IOException("Ungültige HTTP-Zeile.");
      out.write(value);
    }
  }
}
