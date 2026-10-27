#ifndef SOLIDCRAFT_IPADOS_H
#define SOLIDCRAFT_IPADOS_H
#include <stddef.h>
#include <stdint.h>

/* ABI 1. All calls for a session are serialized by the host. Handles are opaque
 * integers; zero is invalid. No Rust pointers or allocations cross the ABI.
 * Return 0 = success, -1 = failure (read JSON response), -2 = invalid handle.
 * Create returns zero on failure. Destroy releases renderer before engine.
 * Every input/output pointer must refer to valid, nonoverlapping memory for its
 * stated length until the call returns. Requests are UTF-8 JSON, <= 1 MiB.
 * Response bytes remain Rust-owned and are copied, without a NUL terminator.
 * Copy returns -1 if capacity is too small and writes nothing in that case.
 * A handle's response is replaced by its next operation; len/copy are serialized.
 */
uint64_t sc_session_create(void);
int32_t sc_session_destroy(uint64_t session);
int32_t sc_request(uint64_t session, const uint8_t *json, size_t length);
size_t sc_response_len(uint64_t session);
int32_t sc_response_copy(uint64_t session, uint8_t *output, size_t capacity);

/* layer must be a live CAMetalLayer created on the main thread. The host retains
 * it until destroy or another successful attach has returned. UIKit changes its
 * layout on main; calls run on one serial worker except synchronous suspend on
 * resign-active. A host render lock serializes that suspend with all surface
 * calls and response copies. Dimensions are
 * drawable pixels, not points. Zero dimensions suspend drawing. Backgrounding
 * must drain queued draws and call suspend before allowing further GPU work.
 * Attach/resize resume drawing; destroy is mandatory before releasing the layer.
 */
int32_t sc_surface_attach(uint64_t session, void *layer, uint32_t width, uint32_t height);
int32_t sc_surface_resize(uint64_t session, uint32_t width, uint32_t height);
int32_t sc_surface_draw(uint64_t session);
int32_t sc_surface_suspend(uint64_t session);
#endif
