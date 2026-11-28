// Vanilla 1.18: nearest valid mansion per seed, plus the first rejected placement candidate.
import java.util.*;
public class Mansion18 {
  public static void main(String[] args) throws Exception {
    ab.a(); yj.a();
    var reg = gy.b();
    var biomes = reg.d(gx.aR);
    var feature = cyi.b.get("mansion");
    for (long seed : new long[]{0, -1}) {
      var gen = cty.a(reg, seed);
      var cfg = gen.d().a(feature);
      var source = gen.e();
      var rm = new afq(aen.b); rm.a(new aep(null, "minecraft"));
      var level = djn.a(java.nio.file.Path.of("tmp-world")).c("oracle");
      var structs = new dgp(rm, level, ajr.a());
      var heights = caf.e(-64, 384);
      var variants = gen.d().b(feature);
      List<bzk> candidates = new ArrayList<>();
      for (int rx = -24; rx <= 24; rx++)
        for (int rz = -24; rz <= 24; rz++)
          candidates.add(feature.a(cfg, seed, rx * cfg.a(), rz * cfg.a()));
      candidates.sort(Comparator.comparingLong(p -> (long)p.c*p.c + (long)p.d*p.d));
      boolean valid = false, rejected = false;
      for (var pos : candidates) {
        boolean started = false;
        for (var v : variants.keySet()) {
          var st = v.a(reg, gen, source, structs, seed, pos, 0, cfg, heights,
              b -> biomes.c(b).map(variants.get(v)::contains).orElse(false));
          if (st.b() && !st.i().isEmpty() && st.c().equals(pos)) {
            if (!valid) System.out.println("MANSION valid " + seed + " " + pos.c + " " + pos.d);
            valid = true; started = true; break;
          }
        }
        if (!started && !rejected) {
          System.out.println("MANSION rejected " + seed + " " + pos.c + " " + pos.d);
          rejected = true;
        }
        if (valid && rejected) break;
      }
      if (!valid || !rejected) throw new AssertionError("incomplete mansion sample for " + seed);
    }
    System.exit(0);
  }
}
