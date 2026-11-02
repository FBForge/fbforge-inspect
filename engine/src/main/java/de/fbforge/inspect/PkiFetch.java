package de.fbforge.inspect;

import java.io.IOException;

/** Narrow certificate fetch boundary; production always supplies PublicHttp. */
@FunctionalInterface
interface PkiFetch {
  PublicHttp.Response pki(String url, byte[] ocsp) throws IOException;
}
