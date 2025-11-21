#include "obs/obs.h"

#include "obs/callback/signal.h"
#include "obs/callback/calldata.h"
#include "obs/graphics/graphics.h"

#ifdef _WIN32
#include "obs/util/windows/window-helpers.h"
#include "window_capture.h"
#include "game_capture.h"
#include "display_capture.h"
#else
#include "obs/obs-nix-platform.h"
#endif