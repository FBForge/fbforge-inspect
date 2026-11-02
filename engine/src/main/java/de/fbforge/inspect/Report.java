package de.fbforge.inspect;

import com.google.gson.*;
import java.time.Instant;
import java.util.*;

/** Evidence is deliberately separate from the interpretation of that evidence. */
public final class Report {
  public final int protocolVersion = 1;
  public final String appVersion = "0.1.0";
  public final String createdAt = Instant.now().toString();
  public final String operation, environment, apiVersion;
  public final List<Finding> findings = new ArrayList<>();
  public final List<Request> requests = new ArrayList<>();
  public final JsonObject data = new JsonObject();
  public final Map<String, String> rawResponses = new LinkedHashMap<>();

  public record Finding(
      FindingCode code,
      String status,
      String title,
      String detail,
      String path,
      String source,
      String nextStep,
      String limitation) {}

  public record Request(
      String url,
      String method,
      String at,
      int status,
      int bytes,
      String outcome,
      Long durationMs,
      boolean viaProxy) {
    public Request(String url, String method, String at, int status, int bytes, String outcome) {
      this(url, method, at, status, bytes, outcome, null, false);
    }
  }

  public Report(String operation, String environment, String apiVersion) {
    this.operation = operation;
    this.environment = environment;
    this.apiVersion = apiVersion;
  }

  public void add(
      String status,
      String title,
      String detail,
      String path,
      String source,
      String next,
      String limitation) {
    add(FindingCode.GENERAL, status, title, detail, path, source, next, limitation);
  }

  public void add(
      FindingCode code,
      String status,
      String title,
      String detail,
      String path,
      String source,
      String next,
      String limitation) {
    findings.add(new Finding(code, status, title, detail, path, source, next, limitation));
  }

  public void check(boolean pass, String title, String detail, String path, String source) {
    check(FindingCode.GENERAL, pass, title, detail, path, source);
  }

  public void check(
      FindingCode code, boolean pass, String title, String detail, String path, String source) {
    add(
        code,
        pass ? "confirmed" : "error",
        title,
        detail,
        path,
        source,
        pass ? "" : "Veröffentlichte Konfiguration prüfen.",
        "");
  }

  public void unknown(String title, String detail, String path, String source) {
    unknown(FindingCode.GENERAL, title, detail, path, source);
  }

  public void unknown(FindingCode code, String title, String detail, String path, String source) {
    add(
        code,
        "unknown",
        title,
        detail,
        path,
        source,
        "Quelle oder Konfiguration gezielt prüfen.",
        "Mit den verfügbaren öffentlichen Daten nicht nachgewiesen.");
  }
}
