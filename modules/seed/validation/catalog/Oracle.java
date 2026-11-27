// Vanilla Java 1.18: actual Brigadier locate literals + biome-reachable configurations.
import com.mojang.brigadier.CommandDispatcher;
import java.util.*;
public class Oracle {
  public static void main(String[] args) throws Exception {
    ab.a();
    yj.a();
    var reg = gy.b();
    var props = new Properties();
    props.setProperty("level-seed", "1");
    var dims = cty.a(reg, props).d();
    var commands = new CommandDispatcher<dm>();
    aam.a(commands);
    var biomes = reg.d(gx.aR);
    for (var node : commands.getRoot().getChild("locate").getChildren()) {
      String name = node.getName();
      System.out.println("CAT target " + name);
      var ft = cyi.b.get(name);
      if (ft == null)
        throw new AssertionError(name);
      for (var e : dims.e()) {
        var gen = e.getValue().c();
        String dim = e.getKey().a().toString();
        if (gen.d().a(ft) == null)
          continue;
        if (ft == cyi.l) {
          if (gen.d().b() != null) {
            gen.a(new bzk(0, 0));
            var field = cpz.class.getDeclaredField("f");
            field.setAccessible(true);
            if (!((List<?>) field.get(gen)).isEmpty())
              System.out.println("CAT row " + dim + " " + name + " " + id.e.b(lk.a));
          }
          continue;
        }
        var possible = new HashSet<>();
        for (var b : gen.e().b()) possible.add(biomes.c(b).orElseThrow());
        var variants = gen.d().b(ft);
        for (var variant : variants.keySet()) {
          if (Collections.disjoint(variants.get(variant), possible))
            continue;
          var key = id.e.b(variant);
          if (key == null)
            throw new AssertionError("Unnamed vanilla variant of " + name);
          System.out.println("CAT row " + dim + " " + name + " " + key);
        }
      }
    }
  }
}
