// SPDX-License-Identifier: MIT
// God of War III Remastered - Linux & Windows Input Bridge & Mouse Injector

#ifdef _WIN32
#include <windows.h>
#else
#define _GNU_SOURCE
#include <dlfcn.h>
#include <strings.h>
#endif

#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <SDL3/SDL.h>

// Scancodes used for mouse buttons (F13 - F19)
#define SCANCODE_MOUSE_LEFT    104 // F13
#define SCANCODE_MOUSE_RIGHT   105 // F14
#define SCANCODE_MOUSE_MIDDLE  106 // F15
#define SCANCODE_MOUSE_SIDE1   107 // F16
#define SCANCODE_MOUSE_SIDE2   108 // F17
#define SCANCODE_MOUSE_WHEELUP 109 // F18
#define SCANCODE_MOUSE_WHEELDN 110 // F19

typedef const bool* (*RealSDL_GetKeyboardState)(int *numkeys);
typedef SDL_MouseButtonFlags (*RealSDL_GetMouseState)(float *x, float *y);
typedef SDL_MouseButtonFlags (*RealSDL_GetGlobalMouseState)(float *x, float *y);
typedef SDL_Window* (*RealSDL_GetKeyboardFocus)(void);
typedef SDL_Window* (*RealSDL_GetMouseFocus)(void);
typedef bool (*RealSDL_CursorVisible)(void);
typedef bool (*RealSDL_HideCursor)(void);
typedef bool (*RealSDL_ShowCursor)(void);
typedef bool (*RealSDL_PollEvent)(SDL_Event *event);
typedef SDL_Scancode (*RealSDL_GetScancodeFromName)(const char *name);
typedef SDL_Surface* (*RealSDL_CreateSurface)(int width, int height, SDL_PixelFormat format);
typedef void (*RealSDL_DestroySurface)(SDL_Surface *surface);
typedef SDL_Cursor* (*RealSDL_CreateColorCursor)(SDL_Surface *surface, int hot_x, int hot_y);
typedef bool (*RealSDL_SetCursor)(SDL_Cursor *cursor);
typedef SDL_Window* (*RealSDL_CreateWindowWithProperties)(SDL_PropertiesID props);
typedef SDL_Window* (*RealSDL_CreateWindow)(const char *title, int w, int h, SDL_WindowFlags flags);
typedef bool (*RealSDL_ShowWindow)(SDL_Window *window);
typedef bool (*RealSDL_HideWindow)(SDL_Window *window);
typedef bool (*RealSDL_RaiseWindow)(SDL_Window *window);

static RealSDL_GetKeyboardState real_GetKeyboardState = NULL;
static RealSDL_GetMouseState real_GetMouseState = NULL;
static RealSDL_GetGlobalMouseState real_GetGlobalMouseState = NULL;
static RealSDL_GetKeyboardFocus real_GetKeyboardFocus = NULL;
static RealSDL_GetMouseFocus real_GetMouseFocus = NULL;
static RealSDL_CursorVisible real_CursorVisible = NULL;
static RealSDL_HideCursor real_HideCursor = NULL;
static RealSDL_ShowCursor real_ShowCursor = NULL;
static RealSDL_PollEvent real_PollEvent = NULL;
static RealSDL_GetScancodeFromName real_GetScancodeFromName = NULL;
static RealSDL_CreateSurface real_CreateSurface = NULL;
static RealSDL_DestroySurface real_DestroySurface = NULL;
static RealSDL_CreateColorCursor real_CreateColorCursor = NULL;
static RealSDL_SetCursor real_SetCursor = NULL;
static RealSDL_CreateWindowWithProperties real_CreateWindowWithProperties = NULL;
static RealSDL_CreateWindow real_CreateWindow = NULL;
static RealSDL_ShowWindow real_ShowWindow = NULL;
static RealSDL_HideWindow real_HideWindow = NULL;
static RealSDL_RaiseWindow real_RaiseWindow = NULL;

static int hide_cursor_enabled = -1;
static int wait_warmup_enabled = -1;
static volatile bool warmup_done = false;
static SDL_Window* tracked_game_window = NULL;
static volatile uint32_t tracked_mouse_buttons = 0;
static volatile uint32_t latched_mouse_buttons = 0;
static volatile int is_window_focused = 1;
static volatile int wheel_up_ticks = 0;
static volatile int wheel_dn_ticks = 0;
static SDL_Cursor* transparent_cursor = NULL;
static volatile bool cursor_is_hidden = false;

static void apply_cursor_hidden(void) {
    if (!hide_cursor_enabled) return;

    if (!transparent_cursor && real_CreateSurface && real_CreateColorCursor && real_DestroySurface) {
        SDL_Surface* s = real_CreateSurface(1, 1, SDL_PIXELFORMAT_RGBA32);
        if (s) {
            uint32_t* pix = (uint32_t*)s->pixels;
            if (pix) *pix = 0; // 100% transparent pixel
            transparent_cursor = real_CreateColorCursor(s, 0, 0);
            real_DestroySurface(s);
        }
    }

    if (transparent_cursor && real_SetCursor) {
        real_SetCursor(transparent_cursor);
    }
    if (real_HideCursor) {
        real_HideCursor();
    }
    cursor_is_hidden = true;
}

static void init_symbols(void) {
    if (!real_GetKeyboardState) {
#ifdef _WIN32
        HMODULE h = GetModuleHandleA("SDL3.dll");
        if (!h) h = LoadLibraryA("SDL3.dll");
        if (h) {
            real_GetKeyboardState = (RealSDL_GetKeyboardState)GetProcAddress(h, "SDL_GetKeyboardState");
            real_GetMouseState = (RealSDL_GetMouseState)GetProcAddress(h, "SDL_GetMouseState");
            real_GetGlobalMouseState = (RealSDL_GetGlobalMouseState)GetProcAddress(h, "SDL_GetGlobalMouseState");
            real_GetKeyboardFocus = (RealSDL_GetKeyboardFocus)GetProcAddress(h, "SDL_GetKeyboardFocus");
            real_GetMouseFocus = (RealSDL_GetMouseFocus)GetProcAddress(h, "SDL_GetMouseFocus");
            real_CursorVisible = (RealSDL_CursorVisible)GetProcAddress(h, "SDL_CursorVisible");
            real_HideCursor = (RealSDL_HideCursor)GetProcAddress(h, "SDL_HideCursor");
            real_ShowCursor = (RealSDL_ShowCursor)GetProcAddress(h, "SDL_ShowCursor");
            real_PollEvent = (RealSDL_PollEvent)GetProcAddress(h, "SDL_PollEvent");
            real_GetScancodeFromName = (RealSDL_GetScancodeFromName)GetProcAddress(h, "SDL_GetScancodeFromName");
            real_CreateSurface = (RealSDL_CreateSurface)GetProcAddress(h, "SDL_CreateSurface");
            real_DestroySurface = (RealSDL_DestroySurface)GetProcAddress(h, "SDL_DestroySurface");
            real_CreateColorCursor = (RealSDL_CreateColorCursor)GetProcAddress(h, "SDL_CreateColorCursor");
            real_SetCursor = (RealSDL_SetCursor)GetProcAddress(h, "SDL_SetCursor");
            real_CreateWindowWithProperties = (RealSDL_CreateWindowWithProperties)GetProcAddress(h, "SDL_CreateWindowWithProperties");
            real_CreateWindow = (RealSDL_CreateWindow)GetProcAddress(h, "SDL_CreateWindow");
            real_ShowWindow = (RealSDL_ShowWindow)GetProcAddress(h, "SDL_ShowWindow");
            real_HideWindow = (RealSDL_HideWindow)GetProcAddress(h, "SDL_HideWindow");
            real_RaiseWindow = (RealSDL_RaiseWindow)GetProcAddress(h, "SDL_RaiseWindow");
        }
#else
        real_GetKeyboardState = (RealSDL_GetKeyboardState)dlsym(RTLD_NEXT, "SDL_GetKeyboardState");
        if (!real_GetKeyboardState) {
            void* h = dlopen("libSDL3.so.0", RTLD_NOW | RTLD_GLOBAL);
            if (!h) h = dlopen("libSDL3.so", RTLD_NOW | RTLD_GLOBAL);
            if (h) {
                real_GetKeyboardState = (RealSDL_GetKeyboardState)dlsym(h, "SDL_GetKeyboardState");
                real_GetMouseState = (RealSDL_GetMouseState)dlsym(h, "SDL_GetMouseState");
                real_GetGlobalMouseState = (RealSDL_GetGlobalMouseState)dlsym(h, "SDL_GetGlobalMouseState");
                real_GetKeyboardFocus = (RealSDL_GetKeyboardFocus)dlsym(h, "SDL_GetKeyboardFocus");
                real_GetMouseFocus = (RealSDL_GetMouseFocus)dlsym(h, "SDL_GetMouseFocus");
                real_CursorVisible = (RealSDL_CursorVisible)dlsym(h, "SDL_CursorVisible");
                real_HideCursor = (RealSDL_HideCursor)dlsym(h, "SDL_HideCursor");
                real_ShowCursor = (RealSDL_ShowCursor)dlsym(h, "SDL_ShowCursor");
                real_PollEvent = (RealSDL_PollEvent)dlsym(h, "SDL_PollEvent");
                real_GetScancodeFromName = (RealSDL_GetScancodeFromName)dlsym(h, "SDL_GetScancodeFromName");
                real_CreateSurface = (RealSDL_CreateSurface)dlsym(h, "SDL_CreateSurface");
                real_DestroySurface = (RealSDL_DestroySurface)dlsym(h, "SDL_DestroySurface");
                real_CreateColorCursor = (RealSDL_CreateColorCursor)dlsym(h, "SDL_CreateColorCursor");
                real_SetCursor = (RealSDL_SetCursor)dlsym(h, "SDL_SetCursor");
                real_CreateWindowWithProperties = (RealSDL_CreateWindowWithProperties)dlsym(h, "SDL_CreateWindowWithProperties");
                real_CreateWindow = (RealSDL_CreateWindow)dlsym(h, "SDL_CreateWindow");
                real_ShowWindow = (RealSDL_ShowWindow)dlsym(h, "SDL_ShowWindow");
                real_HideWindow = (RealSDL_HideWindow)dlsym(h, "SDL_HideWindow");
                real_RaiseWindow = (RealSDL_RaiseWindow)dlsym(h, "SDL_RaiseWindow");
            }
        } else {
            real_GetMouseState = (RealSDL_GetMouseState)dlsym(RTLD_NEXT, "SDL_GetMouseState");
            real_GetGlobalMouseState = (RealSDL_GetGlobalMouseState)dlsym(RTLD_NEXT, "SDL_GetGlobalMouseState");
            real_GetKeyboardFocus = (RealSDL_GetKeyboardFocus)dlsym(RTLD_NEXT, "SDL_GetKeyboardFocus");
            real_GetMouseFocus = (RealSDL_GetMouseFocus)dlsym(RTLD_NEXT, "SDL_GetMouseFocus");
            real_CursorVisible = (RealSDL_CursorVisible)dlsym(RTLD_NEXT, "SDL_CursorVisible");
            real_HideCursor = (RealSDL_HideCursor)dlsym(RTLD_NEXT, "SDL_HideCursor");
            real_ShowCursor = (RealSDL_ShowCursor)dlsym(RTLD_NEXT, "SDL_ShowCursor");
            real_PollEvent = (RealSDL_PollEvent)dlsym(RTLD_NEXT, "SDL_PollEvent");
            real_GetScancodeFromName = (RealSDL_GetScancodeFromName)dlsym(RTLD_NEXT, "SDL_GetScancodeFromName");
            real_CreateSurface = (RealSDL_CreateSurface)dlsym(RTLD_NEXT, "SDL_CreateSurface");
            real_DestroySurface = (RealSDL_DestroySurface)dlsym(RTLD_NEXT, "SDL_DestroySurface");
            real_CreateColorCursor = (RealSDL_CreateColorCursor)dlsym(RTLD_NEXT, "SDL_CreateColorCursor");
            real_SetCursor = (RealSDL_SetCursor)dlsym(RTLD_NEXT, "SDL_SetCursor");
            real_CreateWindowWithProperties = (RealSDL_CreateWindowWithProperties)dlsym(RTLD_NEXT, "SDL_CreateWindowWithProperties");
            real_CreateWindow = (RealSDL_CreateWindow)dlsym(RTLD_NEXT, "SDL_CreateWindow");
            real_ShowWindow = (RealSDL_ShowWindow)dlsym(RTLD_NEXT, "SDL_ShowWindow");
            real_HideWindow = (RealSDL_HideWindow)dlsym(RTLD_NEXT, "SDL_HideWindow");
            real_RaiseWindow = (RealSDL_RaiseWindow)dlsym(RTLD_NEXT, "SDL_RaiseWindow");
        }
#endif
    }

    if (hide_cursor_enabled == -1) {
        const char *env = getenv("GOW3_HIDE_CURSOR");
        hide_cursor_enabled = (env && (env[0] == '1' || env[0] == 't' || env[0] == 'T')) ? 1 : 0;
    }

    if (wait_warmup_enabled == -1) {
        const char *w_env = getenv("GOW3_WAIT_WARMUP");
        wait_warmup_enabled = (w_env && (w_env[0] == '1' || w_env[0] == 't' || w_env[0] == 'T')) ? 1 : 0;
    }
}

#ifdef _WIN32
__declspec(dllexport)
#endif
SDL_Window* SDL_CreateWindowWithProperties(SDL_PropertiesID props) {
    init_symbols();
    SDL_Window* win = NULL;
    if (real_CreateWindowWithProperties) {
        win = real_CreateWindowWithProperties(props);
    }
    if (win) {
        tracked_game_window = win;
        apply_cursor_hidden();
        if (wait_warmup_enabled && !warmup_done) {
            if (real_HideWindow) {
                real_HideWindow(win);
            }
        }
    }
    return win;
}

#ifdef _WIN32
__declspec(dllexport)
#endif
SDL_Window* SDL_CreateWindow(const char *title, int w, int h, SDL_WindowFlags flags) {
    init_symbols();
    SDL_Window* win = NULL;
    if (real_CreateWindow) {
        win = real_CreateWindow(title, w, h, flags);
    }
    if (win) {
        tracked_game_window = win;
        apply_cursor_hidden();
        if (wait_warmup_enabled && !warmup_done) {
            if (real_HideWindow) {
                real_HideWindow(win);
            }
        }
    }
    return win;
}

#ifdef _WIN32
__declspec(dllexport)
#endif
bool SDL_ShowWindow(SDL_Window *window) {
    init_symbols();
    if (window) {
        tracked_game_window = window;
    }
    if (wait_warmup_enabled && !warmup_done) {
        // Keep window hidden until shader warmup is 100% complete
        if (real_HideWindow && window) {
            real_HideWindow(window);
        }
        return true;
    }
    if (real_ShowWindow && window) {
        return real_ShowWindow(window);
    }
    return true;
}

#ifdef _WIN32
__declspec(dllexport)
#endif
SDL_Scancode SDL_GetScancodeFromName(const char *name) {
    init_symbols();
    if (!name) return SDL_SCANCODE_UNKNOWN;

    // Direct mapping for custom mouse buttons
#ifdef _WIN32
    #define MATCH(s) (_stricmp(name, s) == 0)
#else
    #define MATCH(s) (strcasecmp(name, s) == 0)
#endif

    if (MATCH("LeftButton") || MATCH("Mouse1") || MATCH("LeftClick")) {
        return (SDL_Scancode)SCANCODE_MOUSE_LEFT; // 104 (F13)
    }
    if (MATCH("RightButton") || MATCH("Mouse2") || MATCH("RightClick")) {
        return (SDL_Scancode)SCANCODE_MOUSE_RIGHT; // 105 (F14)
    }
    if (MATCH("MiddleButton") || MATCH("Mouse3") || MATCH("MiddleClick")) {
        return (SDL_Scancode)SCANCODE_MOUSE_MIDDLE; // 106 (F15)
    }
    if (MATCH("SideButtonBack") || MATCH("Mouse4") || MATCH("Side1")) {
        return (SDL_Scancode)SCANCODE_MOUSE_SIDE1; // 107 (F16)
    }
    if (MATCH("SideButtonForward") || MATCH("Mouse5") || MATCH("Side2")) {
        return (SDL_Scancode)SCANCODE_MOUSE_SIDE2; // 108 (F17)
    }
    if (MATCH("MouseWheelUp") || MATCH("WheelUp")) {
        return (SDL_Scancode)SCANCODE_MOUSE_WHEELUP; // 109 (F18)
    }
    if (MATCH("MouseWheelDown") || MATCH("WheelDown")) {
        return (SDL_Scancode)SCANCODE_MOUSE_WHEELDN; // 110 (F19)
    }

#undef MATCH

    if (real_GetScancodeFromName) {
        return real_GetScancodeFromName(name);
    }
    return SDL_SCANCODE_UNKNOWN;
}

#ifdef _WIN32
__declspec(dllexport)
#endif
const bool* SDL_GetKeyboardState(int *numkeys) {
    init_symbols();
    if (!real_GetKeyboardState) return NULL;

    const bool* orig_state = real_GetKeyboardState(numkeys);
    if (!orig_state) return NULL;

    bool* state = (bool*)orig_state;

    // Query mouse buttons & inject into keyboard scancode array
    // tracked_mouse_buttons: continuously held buttons
    // latched_mouse_buttons: quick clicks between poll frames, guaranteed not to be lost
    uint32_t buttons = tracked_mouse_buttons | latched_mouse_buttons;
    latched_mouse_buttons = 0; // consumed by this frame

    if (real_GetMouseState) {
        float mx = 0.0f, my = 0.0f;
        buttons |= real_GetMouseState(&mx, &my);
    }
    if (!buttons && real_GetGlobalMouseState && is_window_focused) {
        float mx = 0.0f, my = 0.0f;
        buttons |= real_GetGlobalMouseState(&mx, &my);
    }

    state[SCANCODE_MOUSE_LEFT]   = (buttons & SDL_BUTTON_LMASK) != 0;
    state[SCANCODE_MOUSE_RIGHT]  = (buttons & SDL_BUTTON_RMASK) != 0;
    state[SCANCODE_MOUSE_MIDDLE] = (buttons & SDL_BUTTON_MMASK) != 0;
    state[SCANCODE_MOUSE_SIDE1]  = (buttons & SDL_BUTTON_X1MASK) != 0;
    state[SCANCODE_MOUSE_SIDE2]  = (buttons & SDL_BUTTON_X2MASK) != 0;

    if (wheel_up_ticks > 0) {
        state[SCANCODE_MOUSE_WHEELUP] = true;
        wheel_up_ticks--;
    }
    if (wheel_dn_ticks > 0) {
        state[SCANCODE_MOUSE_WHEELDN] = true;
        wheel_dn_ticks--;
    }

    return orig_state;
}

#ifdef _WIN32
__declspec(dllexport)
#endif
bool SDL_PollEvent(SDL_Event *event) {
    init_symbols();
    if (!cursor_is_hidden) {
        apply_cursor_hidden();
    }
    if (!real_PollEvent) return false;

    bool result = real_PollEvent(event);
    if (result && event) {
        uint32_t event_type = event->type;

        // Window focus gained / shown
        if (event_type == SDL_EVENT_WINDOW_FOCUS_GAINED || event_type == 514 /* SDL_EVENT_WINDOW_SHOWN */) {
            is_window_focused = 1;
            apply_cursor_hidden();
        } else if (event_type == SDL_EVENT_WINDOW_FOCUS_LOST) {
            is_window_focused = 0;
            tracked_mouse_buttons = 0;
            latched_mouse_buttons = 0;
        }
        // Mouse button press / release
        else if (event_type == SDL_EVENT_MOUSE_BUTTON_DOWN) {
            is_window_focused = 1;
            uint8_t btn = event->button.button;
            if (btn >= 1 && btn <= 5) {
                uint32_t mask = (1u << (btn - 1));
                tracked_mouse_buttons |= mask;
                latched_mouse_buttons |= mask;
            }
        } else if (event_type == SDL_EVENT_MOUSE_BUTTON_UP) {
            uint8_t btn = event->button.button;
            if (btn >= 1 && btn <= 5) {
                tracked_mouse_buttons &= ~(1u << (btn - 1));
            }
        } else if (event_type == SDL_EVENT_MOUSE_WHEEL) {
            if (event->wheel.y > 0.0f) {
                wheel_up_ticks = 2;
            } else if (event->wheel.y < 0.0f) {
                wheel_dn_ticks = 2;
            }
        }
    }
    return result;
}

// Override Gow3Overlay::SetLoading and ImGui::ProgressBar to completely eliminate
// the engine's plain generic loading bar in fullscreen mode, and reveal the window
// once shader warmup is 100% complete.
#ifdef _WIN32
__declspec(dllexport)
#endif
void _ZN11Gow3Overlay10SetLoadingEbjj(bool is_loading, unsigned int current, unsigned int total) {
    init_symbols();
    if (!is_loading || (total > 0 && current >= total)) {
        warmup_done = true;
        if (wait_warmup_enabled && tracked_game_window) {
            if (real_ShowWindow) {
                real_ShowWindow(tracked_game_window);
            }
            if (real_RaiseWindow) {
                real_RaiseWindow(tracked_game_window);
            }
        }
    }
}

#ifdef _WIN32
__declspec(dllexport)
#endif
void _ZN5ImGui11ProgressBarEfRK6ImVec2PKc(float fraction, const void* size_arg, const char* overlay) {
    (void)fraction;
    (void)size_arg;
    (void)overlay;
}

