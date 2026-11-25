// Compile against the pinned external seedatlas-engine. No game files are required.
#include "finders.h"
#include "util.h"
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

typedef struct {
    int version;
    int64_t seed;
    int x, y, z;
    const char *biome;
} BiomeSample;

typedef struct {
    int version;
    int64_t seed;
    int type;
    int chunk_x, chunk_z;
} StructureSample;

static const BiomeSample biomes[] = {
    {MC_1_12_2, 0, 0, 64, 0, "forest"},
    {MC_1_12_2, -1, 0, 64, 0, "deep_ocean"},
    {MC_1_18, 0, 0, 64, 0, "river"},
    {MC_1_18, -1, 0, 64, 0, "deep_ocean"},
    {MC_26_3, 0, 0, 64, 0, "river"},
    {MC_26_3, -1, 0, 64, 0, "deep_ocean"},
    {MC_26_3, 0, 2784, 64, -1280, "dappled_forest"},
    {MC_26_3, -1, 672, 64, 1984, "dappled_forest"},
    {MC_1_12_2, 12345, 0, 64, 0, "taiga_mountains"},
    {MC_1_18, 12345, 0, 64, 0, "ocean"},
    {MC_26_3, 12345, 0, 64, 0, "ocean"},
    {MC_26_3, 12345, 608, 64, -736, "dappled_forest"},
};

static const StructureSample starts[] = {
    {MC_1_12_2, 0, Village, -25, 21},
    {MC_1_12_2, -1, Village, 10, -56},
    {MC_1_18, 0, Village, 38, 45},
    {MC_1_18, -1, Village, 58, -48},
    {MC_26_3, 0, Village, 38, 45},
    {MC_26_3, -1, Village, 58, -48},
    {MC_26_3, 0, Abandoned_Camp, 11, -11},
    {MC_26_3, -1, Abandoned_Camp, 53, 0},
    {MC_1_12_2, 12345, Village, -43, 5},
    {MC_1_18, 12345, Village, -9, -52},
    {MC_26_3, 12345, Village, -9, -52},
    {MC_26_3, 12345, Abandoned_Camp, 39, -28},
};

static int region_for_chunk(int chunk, int size)
{
    // C truncates toward zero. Placement regions use floor division.
    return chunk / size - (chunk % size < 0);
}

static int predicts_start(Generator *g, const StructureSample *s)
{
    StructureConfig config;
    Pos pos;
    if (!getStructureConfig(s->type, g->mc, &config) || config.regionSize <= 0)
        return 0;
    int rx = region_for_chunk(s->chunk_x, config.regionSize);
    int rz = region_for_chunk(s->chunk_z, config.regionSize);
    return getStructurePos(s->type, g->mc, g->seed, rx, rz, &pos)
        && pos.x == s->chunk_x * 16 && pos.z == s->chunk_z * 16
        && isViableStructurePos(s->type, g, pos.x, pos.z, 0)
        && isViableStructureTerrain(s->type, g, pos.x, pos.z);
}

int main(void)
{
    int failures = 0;
    Generator g;
    for (size_t i = 0; i < sizeof(biomes) / sizeof(biomes[0]); ++i) {
        const BiomeSample *s = &biomes[i];
        setupGenerator(&g, s->version, 0);
        applySeed(&g, DIM_OVERWORLD, (uint64_t)s->seed);
        const char *actual = biome2str(g.mc, getBiomeAt(&g, 1, s->x, s->y, s->z));
        if (!actual || strcmp(actual, s->biome) != 0) {
            fprintf(stderr, "version=%d seed=%" PRId64 " (%d,%d,%d): expected %s, got %s\n",
                    g.mc, s->seed, s->x, s->y, s->z, s->biome, actual ? actual : "(unknown)");
            ++failures;
        }
    }
    for (size_t i = 0; i < sizeof(starts) / sizeof(starts[0]); ++i) {
        const StructureSample *s = &starts[i];
        setupGenerator(&g, s->version, 0);
        applySeed(&g, DIM_OVERWORLD, (uint64_t)s->seed);
        if (!predicts_start(&g, s)) {
            fprintf(stderr, "version=%d seed=%" PRId64 " type=%d: missing start (%d,%d)\n",
                    g.mc, s->seed, s->type, s->chunk_x, s->chunk_z);
            ++failures;
        }
    }
    printf("%zu biome samples, %zu structure starts, %d failed\n",
           sizeof(biomes) / sizeof(biomes[0]), sizeof(starts) / sizeof(starts[0]), failures);
    return failures != 0;
}
