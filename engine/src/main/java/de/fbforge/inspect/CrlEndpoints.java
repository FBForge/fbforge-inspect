package de.fbforge.inspect;

import java.net.URI;
import java.util.*;
import org.bouncycastle.asn1.x509.*;

/** Only full, direct distribution points; no more than two URI locations per certificate. */
final class CrlEndpoints {
  private CrlEndpoints() {}

  static List<String> locations(CRLDistPoint points) {
    List<String> locations = new ArrayList<>();
    if (points == null) return locations;
    for (DistributionPoint point : points.getDistributionPoints()) {
      if (point.getReasons() != null
          || point.getCRLIssuer() != null
          || point.getDistributionPoint() == null
          || point.getDistributionPoint().getType() != DistributionPointName.FULL_NAME) continue;
      for (GeneralName name :
          GeneralNames.getInstance(point.getDistributionPoint().getName()).getNames()) {
        if (name.getTagNo() != GeneralName.uniformResourceIdentifier) continue;
        String location = name.getName().toString();
        try {
          PublicHttp.validateUri(URI.create(location), true);
        } catch (IllegalArgumentException unsupported) {
          continue;
        }
        if (locations.contains(location)) continue;
        locations.add(location);
        if (locations.size() == 2) return locations;
      }
    }
    return locations;
  }
}
