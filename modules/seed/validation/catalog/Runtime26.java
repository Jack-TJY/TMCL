// Optional independent cross-check inside a local vanilla 26.3 normal world.
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.Level;

public final class Runtime26 {
    public static void sample(MinecraftServer server) {
        var dispatcher = server.getCommands().getDispatcher();
        for (var level : server.getAllLevels()) {
            String dim = level.dimension().equals(Level.OVERWORLD) ? "minecraft:overworld" :
                level.dimension().equals(Level.NETHER) ? "minecraft:the_nether" :
                level.dimension().equals(Level.END) ? "minecraft:the_end" : null;
            if (dim == null) throw new AssertionError("not a normal dimension");
            var registry = level.registryAccess().lookupOrThrow(Registries.STRUCTURE);
            var possible = level.getChunkSource().getGenerator().getBiomeSource().possibleBiomes();
            if (level.dimension().equals(Level.OVERWORLD)) {
                for (var tag : registry.listTags().toList()) {
                    String name = "#" + tag.key().location();
                    var cmd = dispatcher.parse("locate structure " + name, server.createCommandSourceStack());
                    if (cmd.getReader().canRead() || cmd.getContext().getCommand()==null) throw new AssertionError(name);
                    for (var member : tag) System.out.println("GROUP " + name + " " + registry.getKey(member.value()));
                }
            }
            for (var key : registry.keySet()) {
                var entry = registry.getOrThrow(ResourceKey.create(Registries.STRUCTURE, key));
                var cmd = dispatcher.parse("locate structure " + key, server.createCommandSourceStack().withLevel(level));
                if (cmd.getReader().canRead() || cmd.getContext().getCommand()==null) throw new AssertionError(key);
                if (entry.value().biomes().stream().anyMatch(possible::contains))
                    System.out.println("RUNTIME " + dim + " " + key);
            }
        }
    }
}
