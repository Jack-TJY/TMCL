/* TMCL: the only engine ABI exposed to Rust. Default overworld, block Y=64. */
#include "finders.h"
#include <stdint.h>
#include <stdlib.h>

/* No terrain spline tables in this build. Never publish output from an
 * unsupported evaluation, nor abort the launcher process on such a query. */
static _Thread_local int missing_spline;
float getTerrainFactor(int amplified, const float p[4]) {
  (void)amplified;
  (void)p;
  missing_spline = 1;
  return 0;
}
float getTerrainJaggedness(int amplified, const float p[4]) {
  return getTerrainFactor(amplified, p);
}

static int version_id(int v) {
  switch (v) {
  case 0:
    return MC_1_12_2;
  case 1:
    return MC_1_18;
  case 2:
    return MC_26_3;
  default:
    return -1;
  }
}
static Generator *create_generator(int mc, int dim, int64_t seed) {
  Generator *g = malloc(sizeof(*g));
  if (g) {
    missing_spline = 0;
    setupGenerator(g, mc, 0);
    applySeed(g, dim, (uint64_t)seed);
  }
  return g;
}
static int valid_axis(int32_t origin, int32_t n, int32_t step) {
  return n > 0 && n <= 256 && origin >= -30000000 &&
         (int64_t)origin + (int64_t)(n - 1) * step <= 30000000;
}
int tmcl_seed_biomes(int32_t v, int32_t dim, int64_t seed, int32_t x, int32_t z,
                     int32_t step, int32_t w, int32_t h, uint8_t *out) {
  int mc = version_id(v);
  if (mc < 0 || !out || step < 1 || step > 4096 || !valid_axis(x, w, step) ||
      !valid_axis(z, h, step) || (dim != DIM_OVERWORLD && dim != DIM_NETHER &&
                                 dim != DIM_END))
    return 1;
  Generator *g = create_generator(mc, dim, seed);
  if (!g)
    return 2;
  int rc = 0;
  Range r = {1, x, z, w, h, 64, 1};
  int *cache = NULL;
  if (step == 1) {
    cache = allocCache(g, r);
    if (!cache || genBiomes(g, cache, r))
      rc = 2;
  }
  for (int row = 0; !rc && row < h; ++row)
    for (int col = 0; col < w; ++col) {
      int i = row * w + col;
      int id = cache ? cache[i]
                     : getBiomeAt(g, 1, x + col * step, 64, z + row * step);
      if (id < 0 || id > UINT8_MAX || missing_spline) {
        rc = 2;
        break;
      }
      out[i] = (uint8_t)id;
    }
  free(cache);
  free(g);
  return rc;
}

typedef struct {
  int32_t chunk_x, chunk_z;
} Start;
static int region(int chunk, int size) {
  return chunk / size - (chunk % size < 0);
}
// Legacy biome relocation consumes the same RNG used for subsequent rings.
// Preserve its exact prefix across queries, bounded to one seed per thread.
static int legacy_strongholds(Generator *g, int x0, int z0, int x1, int z1,
                              Start *out, size_t capacity, size_t *written) {
  static _Thread_local struct {
    int version, count;
    uint64_t seed;
    StrongholdIter iter;
    Start starts[128];
  } cache;
  *written = 0;
  // In all eight rings the radius is at most 128 + 7*192 + 40 chunks;
  // +/-112 blocks for biome relocation still leaves every point within 1536.
  if (x0 > 1536 || x1 < -1536 || z0 > 1536 || z1 < -1536)
    return 0;
  if (cache.version != g->mc || cache.seed != g->seed) {
    cache.version = g->mc;
    cache.seed = g->seed;
    cache.count = 0;
    initFirstStronghold(&cache.iter, g->mc, g->seed);
  }
  int64_t dx = abs(x0) > abs(x1) ? abs(x0) : abs(x1);
  int64_t dz = abs(z0) > abs(z1) ? abs(z0) : abs(z1);
  for (int i = 0; i < 128; i++) {
    if (i == cache.count) {
      // Smallest radius in this or any later ring, minus a conservative
      // 16 chunks for rounding and the biome relocation in both axes.
      int64_t min_r = 128 + 192 * cache.iter.ringnum - 40 - 16;
      if (min_r * min_r > dx * dx + dz * dz)
        break;
      nextStronghold(&cache.iter, g);
      if (missing_spline) {
        cache.version = -1;
        return 2;
      }
      cache.starts[cache.count++] =
          (Start){region(cache.iter.pos.x, 16), region(cache.iter.pos.z, 16)};
    }
    Start p = cache.starts[i];
    if (p.chunk_x < x0 || p.chunk_x > x1 || p.chunk_z < z0 || p.chunk_z > z1)
      continue;
    if (*written == capacity)
      return 2;
    out[(*written)++] = p;
  }
  return 0;
}
// Since 1.19.3 the biome search uses a separate placement RNG: skipping it
// for a distant ring point does not change the rest of the ring sequence.
// Do not use this fast path for earlier versions, where the RNG is shared.
static int stronghold_starts(Generator *g, int x0, int z0, int x1, int z1,
                             Start *out, size_t capacity, size_t *written) {
  if (g->mc <= MC_1_19_2)
    return legacy_strongholds(g, x0, z0, x1, z1, out, capacity, written);
  StrongholdIter iter;
  initFirstStronghold(&iter, g->mc, g->seed);
  size_t n = 0;
  for (int idx = 0; idx < 128; ++idx) {
    int cx = region(iter.nextapprox.x, 16), cz = region(iter.nextapprox.z, 16);
    // 112-block biome relocation, rounded out to a whole chunk on each side.
    int nearby = cx >= x0 - 8 && cx <= x1 + 8 && cz >= z0 - 8 && cz <= z1 + 8;
    nextStronghold(&iter, nearby ? g : NULL);
    if (!nearby)
      continue;
    cx = region(iter.pos.x, 16);
    cz = region(iter.pos.z, 16);
    if (cx < x0 || cx > x1 || cz < z0 || cz > z1)
      continue;
    if (n == capacity)
      return 2;
    out[n++] = (Start){cx, cz};
  }
  if (missing_spline)
    return 2;
  *written = n;
  return 0;
}
int tmcl_seed_structures(int32_t v, int32_t dim, int64_t seed, int32_t kind, int32_t x0,
                         int32_t z0, int32_t x1, int32_t z1, Start *out,
                         size_t capacity, size_t *written) {
  int mc = version_id(v);
  if (mc < 0 || (kind < 0 || kind > 21 || (kind == 1 && mc != MC_26_3)) ||
      !out || !written || x0 < -1875000 || z0 < -1875000 || x1 > 1875000 ||
      z1 > 1875000 || x0 > x1 || z0 > z1 || (int64_t)x1 - x0 >= 256 ||
      (int64_t)z1 - z0 >= 256)
    return 1;
  const int expected_dim = (kind == 15 || kind == 16 || kind == 20) ? DIM_NETHER
                           : kind == 21 ? DIM_END : DIM_OVERWORLD;
  if ((dim != DIM_OVERWORLD && dim != DIM_NETHER && dim != DIM_END) ||
      dim != expected_dim)
    return 1;
  Generator *g = create_generator(mc, dim, seed);
  if (!g)
    return 2;
  if (kind == 2) {
    int rc = stronghold_starts(g, x0, z0, x1, z1, out, capacity, written);
    free(g);
    return rc;
  }
  if (kind == 21) {
    // End cities need the rotated 5x5 minimum-height rule in addition to the
    // biome check; isViableStructureTerrain() does not cover this case.
    SurfaceNoise surface;
    initSurfaceNoise(&surface, DIM_END, (uint64_t)seed);
    StructureConfig cfg;
    if (!getStructureConfig(End_City, mc, &cfg) || cfg.regionSize <= 0) {
      free(g);
      return 2;
    }
    size_t n = 0;
    int rc = 0;
    for (int rz = region(z0, cfg.regionSize);
         !rc && rz <= region(z1, cfg.regionSize); ++rz) {
      for (int rx = region(x0, cfg.regionSize); rx <= region(x1, cfg.regionSize);
           ++rx) {
        Pos p;
        if (!getStructurePos(End_City, mc, (uint64_t)seed, rx, rz, &p))
          continue;
        int cx = region(p.x, 16), cz = region(p.z, 16);
        if (cx < x0 || cx > x1 || cz < z0 || cz > z1)
          continue;
        if (!isViableStructurePos(End_City, g, p.x, p.z, 0) ||
            isViableEndCityTerrain(g, &surface, p.x, p.z) <= 0)
          continue;
        if (n >= capacity) {
          rc = 2;
          break;
        }
        out[n++] = (Start){cx, cz};
      }
    }
    free(g);
    if (rc || missing_spline)
      return 2;
    *written = n;
    return 0;
  }
  if (kind == 17) {
    // Mineshafts use a per-chunk RNG instead of a region grid.
    size_t area = (size_t)(x1 - x0 + 1) * (size_t)(z1 - z0 + 1);
    Pos *candidates = malloc(sizeof(Pos) * area);
    size_t m = 0;
    int rc = candidates ? 0 : 2;
    int found = 0;
    if (candidates)
      found = getMineshafts(mc, (uint64_t)seed, x0, z0, x1, z1, candidates,
                            (int)area);
    for (int i = 0; !rc && i < found && i < (int)area; ++i) {
      Pos p = candidates[i];
      if (!isViableStructurePos(Mineshaft, g, p.x, p.z, 0) ||
          !isViableStructureTerrain(Mineshaft, g, p.x, p.z))
        continue;
      if (m >= capacity) {
        rc = 2;
        break;
      }
      out[m++] = (Start){region(p.x, 16), region(p.z, 16)};
    }
    free(candidates);
    free(g);
    if (rc || missing_spline)
      return 2;
    *written = m;
    return 0;
  }
  const int placements[] = {Village,        Abandoned_Camp, -1,
                            Desert_Pyramid, Jungle_Temple,  Monument,
                            Mansion,        Igloo,          Swamp_Hut,
                            Outpost,        Shipwreck,      Ancient_City,
                            Trail_Ruins,    Trial_Chambers, Treasure,
                            Fortress,       Bastion,        Mineshaft,
                            Ocean_Ruin,     Ruined_Portal,  Ruined_Portal_N,
                            End_City};
  int type = placements[kind];
  StructureConfig cfg;
  if (!getStructureConfig(type, mc, &cfg) || cfg.regionSize <= 0) {
    free(g);
    return 2;
  }
  size_t n = 0;
  int rc = 0;
  for (int rz = region(z0, cfg.regionSize);
       !rc && rz <= region(z1, cfg.regionSize); ++rz) {
    for (int rx = region(x0, cfg.regionSize); rx <= region(x1, cfg.regionSize);
         ++rx) {
      Pos p;
      if (!getStructurePos(type, mc, (uint64_t)seed, rx, rz, &p))
        continue;
      // World positions are block coordinates; C division truncates toward
      // zero, which maps negative non-aligned positions to the wrong chunk.
      int cx = region(p.x, 16), cz = region(p.z, 16);
      if (cx < x0 || cx > x1 || cz < z0 || cz > z1)
        continue;
      if (isViableStructurePos(type, g, p.x, p.z, 0) &&
          isViableStructureTerrain(type, g, p.x, p.z)) {
        if (n >= capacity) {
          rc = 2;
          break;
        }
        out[n++] = (Start){cx, cz};
      }
    }
  }
  free(g);
  if (rc || missing_spline)
    return 2;
  *written = n;
  return 0;
}