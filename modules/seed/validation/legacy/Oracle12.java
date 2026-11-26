// Offline test harness for a locally supplied vanilla server. Not shipped with the app.
public class Oracle12 {
  static class World extends amu {
    final anl biomes;
    World(bfd info) {
      super(null, info, ayn.a(0).d(), new rl(), false);
      biomes = new anl(info);
      this.z = new bfm(null);
    }
    public anl C() {
      return biomes;
    }
    public anh b(et pos) {
      return biomes.a(pos);
    }
    protected axr n() {
      throw new UnsupportedOperationException();
    }
    protected boolean a(int x, int z, boolean empty) {
      return false;
    }
  }
  static void report(String id, bby st, int x, int z) {
    if (!st.a() || st.c().isEmpty() || st.e() != x || st.f() != z)
      throw new AssertionError(id);
    System.out.println("ORACLE " + id + " " + x + " " + z + " pieces=" + st.c().size());
  }
  static class Hold extends bbs {
    void verify(World w, int bx, int bz) {
      this.g = w;
      et p = a(w, new et(bx, 64, bz), false);
      int x = p.p() >> 4, z = p.r() >> 4;
      this.g = w;
      azb.a(w.Q(), f, x, z);
      f.nextInt();
      if (!a(x, z))
        throw new AssertionError();
      report("stronghold", b(x, z), x, z);
    }
  }
  static class Monument extends bbo {
    void verify(World w) {
      et p = a(w, new et(0, 64, 0), false);
      int x = p.p() >> 4, z = p.r() >> 4;
      this.g = w;
      azb.a(w.Q(), f, x, z);
      f.nextInt();
      if (!a(x, z))
        throw new AssertionError();
      report("monument", b(x, z), x, z);
    }
  }
  static class Temples extends bbq {
    void verify(World w) {
      boolean d = false, j = false;
      this.g = w;
      for (int r = 0; r <= 12; r++)
        for (int dx = -r; dx <= r; dx++)
          for (int dz = -r; dz <= r; dz++) {
            if (Math.max(Math.abs(dx), Math.abs(dz)) != r)
              continue;
            et p = a(w, new et(dx * 2048, 64, dz * 2048), false);
            if (p == null)
              continue;
            int x = p.p() >> 4, z = p.r() >> 4;
            azb.a(w.Q(), f, x, z);
            f.nextInt();
            if (!a(x, z))
              throw new AssertionError();
            bby st = b(x, z);
            String part = st.c().get(0).getClass().getName();
            if (!d && part.equals("bbr$a")) {
              report("desert_pyramid", st, x, z);
              d = true;
            }
            if (!j && part.equals("bbr$c")) {
              report("jungle_pyramid", st, x, z);
              j = true;
            }
            if (d && j)
              return;
          }
      throw new AssertionError("no matching pyramid");
    }
  }
  static void writeRings(Hold h, long seed) throws Exception {
    for (java.lang.reflect.Field field : bbs.class.getDeclaredFields())
      if (field.getType() == amn[].class) {
        field.setAccessible(true);
        amn[] points = (amn[]) field.get(h);
        if (points.length != 128)
          throw new AssertionError();
        try (java.io.PrintWriter out = new java.io.PrintWriter("rings-" + seed + ".txt", "UTF-8")) {
          for (amn pos : points) out.println(pos.a + "," + pos.b);
        }
        return;
      }
    throw new AssertionError("missing rings");
  }
  public static void main(String[] args) throws Exception {
    ni.c();
    for (long seed : new long[] {0, -1}) {
      World w = new World(new bfd(new amx(seed, ams.a(0), true, false, amz.b), "qa"));
      System.out.println("ORACLE seed=" + w.Q());
      Hold h = new Hold();
      h.verify(w, 0, 0);
      h.verify(w, -22000, 8000);
      writeRings(h, seed);
      new Monument().verify(w);
      new Temples().verify(w);
    }
  }
}
