// Vanilla Java 1.12.2: enumerate command completions, then query each dimension's generator.
import java.util.*;
public class Catalog12 {
  static class World extends amu {
    World(int id) {
      super(null, new bfd(new amx(0L, ams.a(0), true, false, amz.b), "catalog"), ayn.a(id).d(),
          new rl(), false);
      this.z = new bfm(null);
      this.s.a(this);
    }
    public anh b(et p) {
      return C().a(p);
    }
    protected axr n() {
      throw new UnsupportedOperationException();
    }
    protected boolean a(int x, int z, boolean empty) {
      return false;
    }
  }
  public static void main(String[] args) throws Exception {
    ni.c();
    List<String> names = new cr().a(null, null, new String[] {""}, new et(0, 64, 0));
    for (String name : names) System.out.println("CAT target " + name);
    for (int dim : new int[] {0, -1, 1}) {
      World w = new World(dim);
      axq gen = w.s.c();
      // The old stronghold locator expects its MapGenBase world to have been attached.
      for (java.lang.reflect.Field f : gen.getClass().getDeclaredFields())
        if (azb.class.isAssignableFrom(f.getType())) {
          f.setAccessible(true);
          azb g = (azb) f.get(gen);
          if (g != null)
            g.g = w;
        }
      for (String name : names) {
        et pos = gen.a(w, name, new et(0, 64, 0), false);
        if (pos != null)
          System.out.println("CAT row "
              + (dim == 0         ? "minecraft:overworld"
                      : dim == -1 ? "minecraft:the_nether"
                                  : "minecraft:the_end")
              + " " + name + " " + name);
      }
    }
  }
}
