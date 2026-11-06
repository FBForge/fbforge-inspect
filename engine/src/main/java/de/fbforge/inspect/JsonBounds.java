package de.fbforge.inspect;

import com.google.gson.JsonElement;
import java.util.ArrayDeque;

/** Bound structured request traversal before recursive input inspectors run. */
final class JsonBounds {
  private record Entry(JsonElement value, int depth) {}

  static void check(JsonElement value) {
    var pending = new ArrayDeque<Entry>();
    pending.add(new Entry(value, 0));
    int count = 0;
    while (!pending.isEmpty()) {
      Entry entry = pending.removeLast();
      if (++count > 200000 || entry.depth() > 48)
        throw new IllegalArgumentException("JSON-Eingabe überschreitet Struktur- oder Tiefenlimit.");
      if (entry.value().isJsonObject())
        for (var child : entry.value().getAsJsonObject().entrySet())
          pending.add(new Entry(child.getValue(), entry.depth() + 1));
      else if (entry.value().isJsonArray())
        for (var child : entry.value().getAsJsonArray())
          pending.add(new Entry(child, entry.depth() + 1));
    }
  }
}
