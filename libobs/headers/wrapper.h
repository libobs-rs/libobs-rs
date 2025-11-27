#include "obs/obs.h"

#include "obs/callback/calldata.h"
#include "obs/callback/signal.h"
#include "obs/graphics/graphics.h"

#ifdef _WIN32
#include "display_capture.h"
#include "game_capture.h"
// #include "obs/util/windows/window-helpers.h"

#ifdef __cplusplus
extern "C" {
#endif

enum window_priority {
  WINDOW_PRIORITY_CLASS,
  WINDOW_PRIORITY_TITLE,
  WINDOW_PRIORITY_EXE,
};

enum window_search_mode {
  INCLUDE_MINIMIZED,
  EXCLUDE_MINIMIZED,
};

#ifdef __cplusplus
}
#endif

#include "window_capture.h"

#else
#include "obs/obs-nix-platform.h"
#endif