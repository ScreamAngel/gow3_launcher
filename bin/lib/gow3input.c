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

static int hide_cursor_enabled = -1;
static volatile uint32_t tracked_mouse_buttons = 0;
static volatile uint32_t latched_mouse_buttons = 0;
static volatile int is_window_focused = 1;
static volatile int wheel_up_ticks = 0;
static volatile int wheel_dn_ticks = 0;

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
        }
#endif
    }

    if (hide_cursor_enabled == -1) {
        const char *env = getenv("GOW3_HIDE_CURSOR");
        hide_cursor_enabled = (env && (env[0] == '1' || env[0] == 't' || env[0] == 'T')) ? 1 : 0;
        if (hide_cursor_enabled && real_HideCursor) {
            real_HideCursor();
        }
    }
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

    // Automatic mouse cursor management:
    // Hidden when the game window is focused, visible when focus is lost (Alt-Tab, etc.)
    if (hide_cursor_enabled && real_CursorVisible && real_HideCursor && real_ShowCursor) {
        if (is_window_focused) {
            if (real_CursorVisible()) {
                real_HideCursor();
            }
        } else {
            if (!real_CursorVisible()) {
                real_ShowCursor();
            }
        }
    }

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
    if (!real_PollEvent) return false;

    bool result = real_PollEvent(event);
    if (result && event) {
        uint32_t event_type = event->type;

        // Window focus gained / lost
        if (event_type == SDL_EVENT_WINDOW_FOCUS_GAINED) {
            is_window_focused = 1;
            if (hide_cursor_enabled && real_HideCursor) {
                real_HideCursor();
            }
        } else if (event_type == SDL_EVENT_WINDOW_FOCUS_LOST) {
            is_window_focused = 0;
            tracked_mouse_buttons = 0;
            latched_mouse_buttons = 0;
            if (hide_cursor_enabled && real_ShowCursor) {
                real_ShowCursor();
            }
        } else if (event_type == 514 /* SDL_EVENT_WINDOW_SHOWN */ || event_type == 516 /* SDL_EVENT_WINDOW_EXPOSED */) {
            if (hide_cursor_enabled && is_window_focused && real_HideCursor) {
                real_HideCursor();
            }
        }
        // Mouse button press / release
        else if (event_type == SDL_EVENT_MOUSE_BUTTON_DOWN) {
            is_window_focused = 1;
            if (hide_cursor_enabled && real_HideCursor && real_CursorVisible && real_CursorVisible()) {
                real_HideCursor();
            }
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
