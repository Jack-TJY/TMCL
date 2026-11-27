// Manual 26.3 oracle. Compile against a locally installed vanilla client;
// call sample on its integrated server thread, with a normal overworld.
// No TMCL engine code or candidate coordinates are used.
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderSet;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.chunk.status.ChunkStatus;

public final class VanillaStructure26 {
    public static void sample(ServerLevel level, String id, int fromX, int fromZ) {
        var registry = level.registryAccess().lookupOrThrow(Registries.STRUCTURE);
        var key = ResourceKey.create(Registries.STRUCTURE, Identifier.parse("minecraft:" + id));
        var type = registry.getOrThrow(key);
        var hit = level.getChunkSource().getGenerator().findNearestMapStructure(
            level, HolderSet.direct(type), new BlockPos(fromX, 64, fromZ), 100, false);
        if (hit == null) throw new AssertionError("not found: " + id);
        BlockPos locate = hit.getFirst();
        var chunk = level.getChunk(locate.getX() >> 4, locate.getZ() >> 4,
                                   ChunkStatus.STRUCTURE_STARTS, true);
        var start = chunk.getStartForStructure(type.value());
        if (start == null || !start.isValid()) throw new AssertionError("invalid start: " + locate);
        System.out.println("seed=" + level.getSeed() + " " + id + " start=" + start.getChunkPos());
    }
}
