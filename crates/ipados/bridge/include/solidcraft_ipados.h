#ifndef SOLIDCRAFT_IPADOS_H
#define SOLIDCRAFT_IPADOS_H
#include <stddef.h>
#include <stdint.h>

/* ABI 3. Three independent lanes: engine request/response, viewport request/response,
 * surface calls/response. Calls within each lane are serialized through len/copy;
 * separate lanes may run concurrently. Suspend never locks engine evaluation.
 * Handles are opaque
 * integers; zero is invalid. No Rust pointers or allocations cross the ABI.
 * Return 0 = success, -1 = failure (read JSON response), -2 = invalid handle.
 * Create returns zero on failure. Destroy releases renderer before engine.
 * Every input/output pointer must refer to valid, nonoverlapping memory for its
 * stated length until the call returns. Requests are UTF-8 JSON, <= 1 MiB.
 * Response bytes remain Rust-owned and are copied, without a NUL terminator.
 * Copy returns -1 if capacity is too small and writes nothing in that case.
 * A lane's response is replaced by its next operation; len/copy are serialized.
 * Destroy must follow host worker completion; it invalidates pending publications.
 */
uint64_t sc_session_create(void);
int32_t sc_session_destroy(uint64_t session);
int32_t sc_request(uint64_t session, const uint8_t *json, size_t length);
size_t sc_response_len(uint64_t session);
int32_t sc_response_copy(uint64_t session, uint8_t *output, size_t capacity);

/* ABI 3 adds source-aware samples/batches, tool/input commands and tool_work.
 * Input positions are canvas-local display points and points*scale pixels; epochs
 * and document/geometry stamps refer to the last presented frame. Predicted
 * samples are explicitly marked and cannot change contact or CAD state.
 * tool_work is an engine-lane request: it consumes one latest preview/commit.
 * Hosts schedule at most one tool_work at a time and poll tool_pending afterward.
 * Display requests: input/input_batch, viewport metrics, view/selection/tool/input commands, snapshot.
 * They never evaluate or tessellate CAD and use a separate worker from sc_request.
 */
int32_t sc_view_request(uint64_t session, const uint8_t *json, size_t length);
size_t sc_view_response_len(uint64_t session);
int32_t sc_view_response_copy(uint64_t session, uint8_t *output, size_t capacity);
size_t sc_surface_response_len(uint64_t session);
int32_t sc_surface_response_copy(uint64_t session, uint8_t *output, size_t capacity);

/* Project files add engine-lane JSON requests (no C ABI signature changes):
 * {kind:"document",source?:path,name,destination:path,repair?:false}: prepare/open, evaluate
 * and persist a candidate through the existing engine commands, then replace
 * the live session. Failed preparation leaves the current document/history intact.
 * Explicit repair:true requires a source and new destination; it cold-evaluates
 * without a model cache and permits reference warnings in the initial scene for
 * review. Evaluation errors and stale results still fail. Subsequent modelling
 * checks remain strict; no reference is silently recaptured.
 * {kind:"save_document",path}: save committed state without replacing the
 * viewport snapshot or cancelling provisional tools/numeric edits.
 */

/* Step 5 adds engine-lane JSON requests (no C ABI signature changes):
 * {kind:"workspace"}: cached document/component/feature/body presentation data,
 * authoritative history availability and supported native tool context.
 * {kind:"numeric",action:"begin"|"preview"|"commit"|"cancel",token?,expression?,dimension?}:
 * an explicit, revisioned parameter transaction. A selected driving sketch
 * dimension edits its existing expression. Selected sketch geometry begins a new
 * driving dimension; optional dimension is the native sketch.dimension type.
 * The initial measured dimension exists only on a scratch session until Apply.
 * Expressions pass unchanged through
 * the engine unit/expression evaluator. Preview never changes the document;
 * commit creates one upstream undo action. Cancel drops only provisional data.
 * Begin returns a numeric token; other actions require it. Selection/tool switches
 * and document commands are disabled until Apply/Cancel. Finger navigation and
 * viewport metric changes remain available. Lifecycle suspend invalidates tokens.
 * View-lane select.entity consumes an existing serialized body/feature/sketch Sel with
 * optional additive selection. It shares the viewport selection and hit testing.
 */

/* Step 6 adds {kind:"sketch_plane",plane:<native sketch.create plane value>} on
 * the engine lane. Resolves the actual datum, construction or attached face on a
 * scratch session, then shows its plane using the shared camera. No document or
 * history mutation. view.cancel_plane restores the prior view; confirmation uses
 * the ordinary sketch.create command with the original attachment value.
 * Sketch Pencil work uses the existing tool_work mailbox and atomic commands.
 * Prediction never enters a commit. tool.cancel cancels only the current tool
 * operation; sketch.finish ends editing; timeline.delete explicitly deletes.
 */

/* Step 7 adds engine-lane modelling transactions without C signature changes:
 * {kind:"model",action:"begin",operation?:<native tool id>,feature?:<feature id>,rollback?:true}
 * begins a new feature or edits the original document feature. It returns a
 * token, native input/target choices, original expressions and parameters.
 * Feature edits temporarily display that history stage, with earlier geometry
 * available for reference picking. Apply rebuilds through the latest feature;
 * Cancel restores the original view. rollback:false retains full-history editing.
 * {kind:"model",action:"preview"|"commit",token,params:<tool parameters>}
 * evaluates on an upstream scratch session; commit revalidates current document
 * and draft ownership, then accepts one logical undoable action. Preview failure
 * invalidates the previous preview. The host must disable Apply before changing
 * parameters and may enable it only for the successful current preview reply.
 * {kind:"model",action:"selection",token} removes the provisional preview so
 * displayed source geometry and picking agree while acquiring more inputs.
 * {kind:"model",action:"cancel",token} drops provisional state only.
 * Document mutations and numeric editing are disabled while this draft exists;
 * finger navigation and viewport resize remain available. Lifecycle suspension,
 * tool cancellation and document replacement invalidate model tokens. Repeating
 * a consumed token cannot create another feature.
 */

/* Step 8 assembly forms reuse kind:model and its document/token/version checks.
 * Replies include the native command result as `reply` and human-readable
 * diagnostics as `feedback`. These remain present in commit/cancel responses
 * after the provisional draft has been consumed, so limits, effective values
 * and no-motion diagnostics are not lost at confirmation.
 */

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
