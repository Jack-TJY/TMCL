// Offline test harness for a locally supplied vanilla server. Not shipped with the app.
import java.nio.file.*;
import java.util.*;
public class Oracle {
  public static void main(String[] args) throws Exception {
    long seed = Long.parseLong(args[0]);
    System.out.println("seed=" + seed);
    ab.a();
    yj.a();
    var reg = gy.b();
    var gen = cty.a(reg, seed);
    var biomes = reg.d(gx.aR);
    var source = gen.e();
    var sampler = gen.c();
    var rm = new afq(aen.b);
    rm.a(new aep(null, "minecraft"));
    var level = djn.a(Path.of("tmp-world")).c("oracle");
    var structs = new dgp(rm, level, ajr.a());
    var heights = caf.e(-64, 384);
    // The vanilla generator lazily evaluates the actual biome-adjusted ring list.
    gen.a(new bzk(0, 0));
    var f = cpz.class.getDeclaredField("f");
    f.setAccessible(true);
    @SuppressWarnings("unchecked") List<bzk> rings = (List<bzk>) f.get(gen);
    if (rings.size() != 128)
      throw new AssertionError("missing rings");
    Files.write(
        Path.of("rings-" + seed + ".txt"), rings.stream().map(pos -> pos.c + "," + pos.d).toList());
    for (var feature : List.of(cyi.l, cyi.g, cyi.f, cyi.m)) {
      var cfg = gen.d().a(feature);
      var variants = gen.d().b(feature);
      String id = feature == cyi.l ? "stronghold"
          : feature == cyi.f       ? "jungle_pyramid"
          : feature == cyi.g       ? "desert_pyramid"
                                   : "monument";

      for (int[] origin :
          feature == cyi.l ? new int[][] {{0, 0}, {-1375, 500}} : new int[][] {{0, 0}}) {
        List<bzk> candidates = new ArrayList<>();
        if (feature == cyi.l)
          candidates.addAll(rings);
        else
          for (int rx = -24; rx <= 24; rx++)
            for (int rz = -24; rz <= 24; rz++)
              candidates.add(feature.a(cfg, seed, rx * cfg.a(), rz * cfg.a()));
        candidates.sort(Comparator.comparingLong(p -> {
          long dx = p.c - origin[0], dz = p.d - origin[1];
          return dx * dx + dz * dz;
        }));
        boolean found = false;
      outer:
        for (var pos : candidates)
          for (var v : feature == cyi.l ? List.of(lk.a) : variants.keySet()) {
            var st = v.a(reg, gen, source, structs, seed, pos, 0, cfg, heights,
                b -> feature == cyi.l || biomes.c(b).map(variants.get(v)::contains).orElse(false));
            if (st.b() && !st.i().isEmpty() && st.c().equals(pos)) {
              System.out.println(
                  "ORACLE " + id + " " + pos.c + " " + pos.d + " pieces=" + st.i().size());
              found = true;
              break outer;
            }
          }
        if (!found)
          throw new AssertionError(id);
      }
    }
    System.exit(0);
  }
}
