package de.fbforge.inspect;

import static org.junit.jupiter.api.Assertions.*;

import java.math.BigInteger;
import java.security.cert.X509Certificate;
import java.util.Date;
import org.bouncycastle.asn1.*;
import org.bouncycastle.asn1.x500.X500Name;
import org.bouncycastle.asn1.x509.*;
import org.bouncycastle.cert.*;
import org.bouncycastle.cert.jcajce.*;
import org.bouncycastle.cert.ocsp.*;
import org.bouncycastle.cert.ocsp.jcajce.JcaBasicOCSPRespBuilder;
import org.bouncycastle.operator.jcajce.*;
import org.junit.jupiter.api.*;

class DelegatedOcspTest {
  @BeforeAll
  static void setup() throws Exception {
    CryptoTest.fixtures();
  }

  X509Certificate delegate(ASN1Encodable noCheck) throws Exception {
    var builder =
        new JcaX509v3CertificateBuilder(
            new X500Name("CN=Synthetic Test CA"),
            BigInteger.valueOf(7),
            Date.from(CryptoTest.now.minusSeconds(3600)),
            Date.from(CryptoTest.now.plusSeconds(86400)),
            new X500Name("CN=Synthetic OCSP Delegate"),
            CryptoTest.otherKey.getPublic());
    builder.addExtension(Extension.keyUsage, true, new KeyUsage(KeyUsage.digitalSignature));
    builder.addExtension(
        Extension.extendedKeyUsage, false, new ExtendedKeyUsage(KeyPurposeId.id_kp_OCSPSigning));
    if (noCheck != null)
      builder.addExtension(new ASN1ObjectIdentifier("1.3.6.1.5.5.7.48.1.5"), false, noCheck);
    return new JcaX509CertificateConverter()
        .getCertificate(
            builder.build(
                new JcaContentSignerBuilder("SHA256withRSA")
                    .build(CryptoTest.issuerKey.getPrivate())));
  }

  byte[] response(X509Certificate signer) throws Exception {
    var builder =
        new JcaBasicOCSPRespBuilder(
            CryptoTest.otherKey.getPublic(),
            new JcaDigestCalculatorProviderBuilder().build().get(CertificateID.HASH_SHA1));
    builder.addResponse(
        CryptoTest.id,
        null,
        Date.from(CryptoTest.now.minusSeconds(60)),
        Date.from(CryptoTest.now.plusSeconds(600)),
        null);
    return new OCSPRespBuilder()
        .build(
            0,
            builder.build(
                new JcaContentSignerBuilder("SHA256withRSA")
                    .build(CryptoTest.otherKey.getPrivate()),
                new X509CertificateHolder[] {new JcaX509CertificateHolder(signer)},
                Date.from(CryptoTest.now)))
        .getEncoded();
  }

  byte[] responderCrl(X509Certificate signer, boolean revoked) throws Exception {
    var builder =
        new X509v2CRLBuilder(
            new X500Name("CN=Synthetic Test CA"), Date.from(CryptoTest.now.minusSeconds(60)));
    builder.setNextUpdate(Date.from(CryptoTest.now.plusSeconds(600)));
    if (revoked)
      builder.addCRLEntry(signer.getSerialNumber(), Date.from(CryptoTest.now.minusSeconds(300)), 1);
    return builder
        .build(
            new JcaContentSignerBuilder("SHA256withRSA").build(CryptoTest.issuerKey.getPrivate()))
        .getEncoded();
  }

  String withCrl(X509Certificate signer, boolean revoked) throws Exception {
    byte[] crl = responderCrl(signer, revoked);
    return Certificates.validateOcsp(
        response(signer),
        CryptoTest.id,
        CryptoTest.issuer,
        CryptoTest.now,
        certificate -> {
          try {
            return Certificates.validateCrl(crl, certificate, CryptoTest.issuer, CryptoTest.now);
          } catch (Exception e) {
            throw new AssertionError(e);
          }
        });
  }

  @Test
  void unprovenDelegateDoesNotConfirmGood() throws Exception {
    assertEquals(
        "unknown",
        Certificates.validateOcsp(
            response(delegate(null)), CryptoTest.id, CryptoTest.issuer, CryptoTest.now));
  }

  @Test
  void revokedDelegateCannotConfirmGood() throws Exception {
    assertEquals("unknown", withCrl(delegate(null), true));
  }

  @Test
  void currentSignedCrlCanEstablishDelegateStatus() throws Exception {
    assertEquals("confirmed", withCrl(delegate(null), false));
  }

  @Test
  void validCaIssuedNoCheckExemptsDelegateRevocation() throws Exception {
    X509Certificate signer = delegate(DERNull.INSTANCE);
    assertTrue(Certificates.hasNoCheck(signer));
    assertEquals(
        "confirmed",
        Certificates.validateOcsp(
            response(signer), CryptoTest.id, CryptoTest.issuer, CryptoTest.now));
  }

  @Test
  void malformedNoCheckDoesNotExemptDelegate() throws Exception {
    assertEquals(
        "unknown",
        Certificates.validateOcsp(
            response(delegate(new DERUTF8String("invalid"))),
            CryptoTest.id,
            CryptoTest.issuer,
            CryptoTest.now));
  }
}
