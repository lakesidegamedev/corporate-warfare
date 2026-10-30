// The engine's GPU map entry points (crates/cw-ffi/src/gpu_map.rs): a plain C interface that sits
// next to the UniFFI one in the same static library. GpuMap.swift is the only caller.
#include <stddef.h>
#include <stdint.h>

uint64_t cw_gpu_open(const char *assets, const char *scenario);
uint64_t cw_gpu_open_layer(const char *assets, const char *scenario, void *layer);
int32_t cw_gpu_last_error(char *buf, size_t len);
int32_t cw_gpu_info(uint64_t id, char *buf, size_t len);
int32_t cw_gpu_update(uint64_t id, const char *json);
int32_t cw_gpu_render(uint64_t id, const double *cam, uint32_t width, uint32_t height, uint32_t scale,
                      uint8_t *out, size_t out_len, double *timing);
int32_t cw_gpu_render_layer(uint64_t id, const double *cam, uint32_t width, uint32_t height, double *timing);
int32_t cw_gpu_project(uint64_t id, const double *cam, float width, float height, const double *pts,
                       size_t n, float *out);
int32_t cw_gpu_pick(uint64_t id, const double *cam, float width, float height, float px, float py,
                    char *buf, size_t len);
int32_t cw_gpu_buildings(uint64_t id, char *buf, size_t len);
int32_t cw_gpu_building_rects(uint64_t id, const double *cam, float width, float height, float *out, size_t max);
void cw_gpu_close(uint64_t id);
