package de.fbforge.inspect;

import com.google.gson.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.*;

/**
 * Portable public diagnostics. No GUI, shell interpolation, or platform-specific process required.
 */
final class Cli {
  static final int MAX_JOB = 2 * 1024 * 1024;
  static final int MAX_RESULT = 16 * 1024 * 1024;

  private Cli() {}

  static int execute(String[] args, PrintStream out, PrintStream err) {
    if (Arrays.equals(args, new String[] {"--help"})) {
      out.println(
          "FBForge Inspect 0.2.0 · öffentliche Serverdiagnose\n"
              + "java -jar inspect-engine.jar --job auftrag.json --output ergebnis.inspect.json\n"
              + "Optional: --proxy-url URL --proxy-user-env VARIABLE --proxy-password-env"
              + " VARIABLE\n"
              + "Optional: --ca-file unternehmens-ca.pem\n"
              + "Exitcodes: 0 abgeschlossen, 2 Fehlerbefund, 3 unvollständig, 64 Eingabe, 70"
              + " intern, 74 Datei.\n"
              + "Keine Fachdaten oder Zugangsdaten in Prüfaufträgen. Ergebnisse vor Weitergabe"
              + " prüfen.");
      return 0;
    }
    if (Arrays.equals(args, new String[] {"--version"})) {
      out.println("FBForge Inspect 0.2.0 · Austauschformat 1");
      return 0;
    }
    try {
      Map<String, String> options = options(args);
      Path source = Path.of(options.get("--job")), target = Path.of(options.get("--output"));
      byte[] bytes = read(source, MAX_JOB);
      JsonObject job =
          JsonParser.parseString(new String(bytes, StandardCharsets.UTF_8)).getAsJsonObject();
      List<JsonObject> requests = PortableJob.requests(job);
      JsonObject network =
          job.has("network") ? job.getAsJsonObject("network").deepCopy() : new JsonObject();
      if (options.containsKey("--proxy-url")) {
        network.addProperty("mode", "manual");
        network.addProperty("proxyUrl", options.get("--proxy-url"));
      }
      for (String field : List.of("user", "password")) {
        String variable = options.get("--proxy-" + field + "-env");
        if (variable != null) {
          if (!variable.matches("[A-Za-z_][A-Za-z0-9_]{0,127}"))
            throw new IllegalArgumentException();
          String value = System.getenv(variable);
          if (value == null) throw new IllegalArgumentException();
          network.addProperty(field.equals("user") ? "username" : "password", value);
        }
      }
      if (options.containsKey("--ca-file"))
        network.addProperty(
            "httpsCaPem",
            new String(
                read(Path.of(options.get("--ca-file")), 256 * 1024), StandardCharsets.UTF_8));
      if (network.size() > 0) NetworkOptions.read(network);
      JsonObject result =
          run(
              job,
              requests,
              network,
              HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes)));
      byte[] encoded = (Main.JSON.toJson(result) + "\n").getBytes(StandardCharsets.UTF_8);
      if (encoded.length > MAX_RESULT) throw new IOException();
      // Never overwrite a previous result or an operator-selected input accidentally.
      Files.write(target, encoded, StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
      int code = result.get("exitCode").getAsInt();
      out.println("Prüfungen beendet. Exitcode " + code + ". Ergebnisdatei in Inspect öffnen.");
      return code;
    } catch (IOException failure) {
      err.println("Datei nicht lesbar, Ergebnis bereits vorhanden oder Dateigröße überschritten.");
      return 74;
    } catch (IllegalArgumentException | IllegalStateException failure) {
      err.println("Ungültiger Prüfauftrag oder ungültige Optionen. --help zeigt den Aufruf.");
      return 64;
    } catch (Exception failure) {
      err.println("Prüflauf nicht abgeschlossen. Keine vertraulichen Fehlerdetails ausgegeben.");
      return 70;
    }
  }

  static JsonObject run(
      JsonObject job, List<JsonObject> requests, JsonObject network, String digest)
      throws Exception {
    JsonObject result = new JsonObject();
    result.addProperty("format", "fbforge-inspect-result");
    result.addProperty("formatVersion", 1);
    result.addProperty("appVersion", "0.2.0");
    result.addProperty("createdAt", Instant.now().toString());
    result.addProperty("jobSha256", digest);
    JsonObject runtime = new JsonObject();
    runtime.addProperty("os", System.getProperty("os.name"));
    runtime.addProperty("architecture", System.getProperty("os.arch"));
    runtime.addProperty("javaVersion", System.getProperty("java.version"));
    result.add("runtime", runtime);
    JsonArray entries = new JsonArray();
    result.add("checks", entries);
    long deadline = System.nanoTime() + TimeUnit.MINUTES.toNanos(10);
    int exit = 0;
    ExecutorService executor = Executors.newVirtualThreadPerTaskExecutor();
    try {
      for (int index = 0; index < requests.size(); index++) {
        JsonObject entry = new JsonObject();
        entry.addProperty(
            "id",
            JsonFields.value(job.getAsJsonArray("checks").get(index).getAsJsonObject(), "id"));
        entries.add(entry);
        long remaining = deadline - System.nanoTime();
        if (remaining <= 0) {
          entry.addProperty("error", "JOB_TIME_LIMIT");
          exit = Math.max(exit, 3);
          continue;
        }
        JsonObject request = requests.get(index);
        if (network.size() > 0) request.add("network", network.deepCopy());
        Future<Report> future = executor.submit(() -> Main.run(request));
        try {
          Report report =
              future.get(Math.min(TimeUnit.SECONDS.toNanos(130), remaining), TimeUnit.NANOSECONDS);
          report.rawResponses.clear();
          JsonObject exported = Main.JSON.toJsonTree(report).getAsJsonObject();
          exported.remove("rawResponses");
          entry.add("report", exported);
          if (report.findings.stream().anyMatch(f -> f.status().equals("error")))
            exit = Math.max(exit, 2);
          if (report.findings.stream().anyMatch(f -> f.status().equals("unknown"))) exit = 3;
        } catch (TimeoutException failure) {
          future.cancel(true);
          entry.addProperty("error", "CHECK_TIME_LIMIT");
          exit = 3;
        } catch (ExecutionException failure) {
          entry.addProperty("error", "CHECK_FAILED");
          exit = 3;
        }
        if (Main.JSON.toJson(result).getBytes(StandardCharsets.UTF_8).length > MAX_RESULT - 4096) {
          entry.remove("report");
          entry.addProperty("error", "RESULT_SIZE_LIMIT");
          exit = 3;
        }
      }
    } finally {
      executor.shutdownNow();
    }
    result.addProperty("exitCode", exit);
    return result;
  }

  static Map<String, String> options(String[] args) {
    Set<String> allowed =
        Set.of(
            "--job",
            "--output",
            "--proxy-url",
            "--proxy-user-env",
            "--proxy-password-env",
            "--ca-file");
    Map<String, String> result = new HashMap<>();
    if (args.length % 2 != 0) throw new IllegalArgumentException();
    for (int i = 0; i < args.length; i += 2)
      if (!allowed.contains(args[i])
          || args[i + 1].isBlank()
          || result.put(args[i], args[i + 1]) != null) throw new IllegalArgumentException();
    if (!result.keySet().containsAll(Set.of("--job", "--output")))
      throw new IllegalArgumentException();
    return result;
  }

  static byte[] read(Path path, int limit) throws IOException {
    try (InputStream stream = Files.newInputStream(path)) {
      byte[] bytes = stream.readNBytes(limit + 1);
      if (bytes.length > limit) throw new IOException();
      return bytes;
    }
  }
}
