package de.fbforge.inspect;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.security.cert.*;
import java.util.*;
import javax.net.ssl.*;

/** Explicit additional HTTPS CAs. Never changes Java defaults or the independent V-PKI store. */
final class HttpsTrust {
  static SSLSocketFactory factory(String pem) {
    if (pem.isBlank()) return (SSLSocketFactory) SSLSocketFactory.getDefault();
    if (pem.length() > 256 * 1024 || pem.contains("PRIVATE KEY"))
      throw new IllegalArgumentException(
          "Nur öffentliche HTTPS-CA-Zertifikate bis 256 KiB zulässig.");
    try {
      var certificates =
          CertificateFactory.getInstance("X.509")
              .generateCertificates(
                  new ByteArrayInputStream(pem.getBytes(StandardCharsets.US_ASCII)));
      if (certificates.isEmpty() || certificates.size() > 32) throw new GeneralSecurityException();
      KeyStore store = KeyStore.getInstance(KeyStore.getDefaultType());
      store.load(null, null);
      int index = 0;
      for (var certificate : certificates) {
        X509Certificate cert = (X509Certificate) certificate;
        if (cert.getBasicConstraints() < 0) throw new GeneralSecurityException();
        cert.checkValidity();
        store.setCertificateEntry("explicit-https-ca-" + (++index), cert);
      }
      X509TrustManager defaults = manager(null), extra = manager(store);
      X509TrustManager combined =
          new X509TrustManager() {
            public X509Certificate[] getAcceptedIssuers() {
              List<X509Certificate> issuers =
                  new ArrayList<>(List.of(defaults.getAcceptedIssuers()));
              issuers.addAll(List.of(extra.getAcceptedIssuers()));
              return issuers.toArray(X509Certificate[]::new);
            }

            public void checkClientTrusted(X509Certificate[] chain, String auth)
                throws CertificateException {
              defaults.checkClientTrusted(chain, auth);
            }

            public void checkServerTrusted(X509Certificate[] chain, String auth)
                throws CertificateException {
              try {
                defaults.checkServerTrusted(chain, auth);
              } catch (CertificateException failure) {
                extra.checkServerTrusted(chain, auth);
              }
            }
          };
      SSLContext context = SSLContext.getInstance("TLS");
      context.init(null, new TrustManager[] {combined}, null);
      return context.getSocketFactory();
    } catch (Exception failure) {
      throw new IllegalArgumentException(
          "Öffentliche HTTPS-CA-Zertifikate sind nicht auswertbar oder nicht gültig.");
    }
  }

  private static X509TrustManager manager(KeyStore store) throws GeneralSecurityException {
    TrustManagerFactory factory =
        TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm());
    factory.init(store);
    for (TrustManager manager : factory.getTrustManagers())
      if (manager instanceof X509TrustManager x509) return x509;
    throw new GeneralSecurityException();
  }
}
