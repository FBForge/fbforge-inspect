package de.fbforge.inspect;

import java.io.IOException;
import java.time.Duration;
import java.util.concurrent.TimeUnit;
import java.util.function.LongSupplier;

/** One diagnosis owns its request, transferred-body and monotonic-time budget. */
final class NetworkBudget {
  static final long MAX_TOTAL = 20L * 1024 * 1024;
  private final LongSupplier clock;
  private final long deadline;
  private int requests;
  private long transferred;

  NetworkBudget(LongSupplier clock) {
    this.clock = clock;
    deadline = clock.getAsLong() + Duration.ofSeconds(120).toNanos();
  }

  void startRequest() throws IOException {
    checkTime();
    if (++requests > 40 || transferred > MAX_TOTAL)
      throw new IOException("Abfragebudget erreicht.");
  }

  void checkTime() throws IOException {
    if (clock.getAsLong() - deadline >= 0) throw new IOException("Zeitlimit erreicht.");
  }

  int timeoutMillis(int ceiling) throws IOException {
    checkTime();
    return (int)
        Math.max(1, Math.min(ceiling, TimeUnit.NANOSECONDS.toMillis(deadline - clock.getAsLong())));
  }

  void requireBodySize(long size) throws IOException {
    checkTime();
    if (size < 0 || size > PublicHttp.MAX_BODY) throw new IOException("Antwortbudget erreicht.");
  }

  void charge(int bytes) throws IOException {
    checkTime();
    if (bytes < 0) throw new IOException("Ungültige Bytezahl.");
    transferred += bytes;
    if (transferred > MAX_TOTAL) throw new IOException("Gesamtantwortbudget erreicht.");
  }

  long remaining() {
    return MAX_TOTAL - transferred;
  }
}
