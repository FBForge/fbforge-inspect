<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from '../contracts';
import type { PublicKeyData, CertificateData } from '../reportData';
import { records } from '../resultValues';
const props = defineProps<{ report: Report }>();

const keys = computed(() => records<PublicKeyData>(props.report.data.keys));
const certificates = computed(() => records<CertificateData>(props.report.data.certificates));
</script>
<template>
  <section v-if="keys.length" class="panel">
    <h3>Öffentliche Schlüssel · JWK / JWKS</h3>
    <article v-for="(key, index) in keys" :key="index" class="key-card">
      <h4 class="mono">{{ key.kid || 'Ohne kid' }}</h4>
      <p>{{ key.algorithm }} · {{ key.bits }} Bit · x5c: {{ key.x5cCount || 0 }} Zertifikate</p>
      <details>
        <summary>JWK-Felder einschließlich x5c anzeigen</summary>
        <pre tabindex="0">{{ JSON.stringify(key.publicJwk, null, 2) }}</pre>
      </details>
    </article>
  </section>
  <div v-if="certificates.length" class="panel">
    <h3>Zertifikatsketten</h3>
    <p class="hint">
      Geprüft werden Schlüsselbindung, Laufzeit, Kette, Vertrauen und Sperrstatus. Die
      JWK-Verwendung wird getrennt geprüft; dies ist keine vollständige V-PKI-Profilprüfung.
    </p>
    <div
      class="table-scroll wide-details-table"
      tabindex="0"
      role="region"
      aria-label="Zertifikatsketten"
    >
      <table>
        <thead>
          <tr>
            <th>Schlüssel / Inhaber</th>
            <th>Aussteller / Laufzeitende</th>
            <th>SHA-256</th>
          </tr>
        </thead>
        <tbody>
          <template v-for="(certificate, index) in certificates" :key="index"
            ><tr>
              <td>
                {{ certificate.subject }}
                <p class="hint">{{ certificate.role }} · x5c[{{ certificate.chainIndex }}]</p>
                <p class="mono hint">{{ certificate.kid }} · {{ certificate.keyPath }}</p>
              </td>
              <td>
                {{ certificate.issuer }}
                <p class="hint">{{ certificate.notAfter }}</p>
              </td>
              <td>
                <span class="mono">{{ certificate.sha256 }}</span>
              </td>
            </tr>
            <tr class="detail-row">
              <td colspan="3">
                <details>
                  <summary>Zertifikat und x5c anzeigen</summary>
                  <p>Gültig ab {{ certificate.notBefore }}</p>
                  <p>
                    Signaturalgorithmus des Zertifikats: {{ certificate.signatureAlgorithm }} ·
                    {{ certificate.signatureAlgorithmOid }}
                  </p>
                  <pre tabindex="0">{{ certificate.pem }}</pre>
                  <p class="hint">x5c[{{ certificate.chainIndex }}] · Base64-DER</p>
                  <pre tabindex="0">{{ certificate.x5c }}</pre>
                </details>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>
  </div>
</template>
