# Shim Function Map

This document maps every shim function in the rehost and explains why it exists.

The shims are the bridge between a Win95 game running in a guest and the browser
page that hosts them. Most calls are no-ops or stubs that answer default values so
the game keeps running; the few that actually do something are highlighted.

## Table of Contents

1. [gdi32.rs](#gdi32rs---gifidll-subset)
2. [user32.rs](#user32rs---userman32dll-subset)
3. [kernel32.rs](#kernel32rs---kernel32dll-subset)
4. [ddraw.rs](#ddrawrs---directdrawdll-subset)
5. [dsound.rs](#dsoundrs---directSOUNDdll-subset)
6. [winmm.rs](#winmmrs---winmmdll-subset)
7. [dinput.rs](#dinputrs---dinputdll-subset)
8. [gdi32.rs — GDI32.dll subset](#gdi32rs---gifidll-subset)
9. [advapi32.rs](#advapi32rs---advapi32dll-subset)
10. [files.rs](#filesrs---filesrs-subset)

---

## gdi32.rs — GDI32.dll subset

This is the most minimal shim: it only implements `GetStockObject`, which the
window class uses for its background brush. The handle it returns is opaque to the
guest — it is passed straight back to `SelectObject`/`DeleteObject` and never
inspected. All other GDI calls (`SelectObject`, `SetBkMode`, `TextOutA`,
`BitBlt`, ...) are deliberately absent; the session stops and names the first one
the guest reaches, so those can be added later.

**Functions:**

| Function | Args | Description |
|---|---|---|
| `GetStockObject` | `(index: u32) -> u32` | Returns a stable handle for stock object indices 0–19 (DC_PEN through DC_BRUSH). Returns 0 for indices beyond 19. Handle = `0x0002_0000 + index * 4`. |

All other GDI calls are unimplemented on purpose.

---

## user32.rs — USER32.dll subset

This shim provides the window infrastructure the game needs: one window class,
one window, and queries about their state. The browser page *is* the window;
there is no border or title bar. The message pump is deliberately minimal —
`GetMessageA` returns `WM_NULL` when the queue is empty to keep the guest alive
instead of ending the process.

**Functions:**

| Function | Args | Description |
|---|---|---|
| `AdjustWindowRectEx` | `(dwFlags: u32, lpRect: *const RECT, bMenu: BOOL, dwStyle: u32) -> BOOL` | No-op; returns 1. The rehost has no border or title bar, so the client rectangle is already the window rectangle. |
| `CreateWindowExA` | `(dwExStyle: u32, lpClassName: *const i8, lpWindowName: *const i8, dwStyle: u32, x: i32, y: i32, nWidth: i32, nHeight: i32, hWndParent: u32, hMenu: u32, hInstance: u32, lpParam: *const u8) -> u32` | Creates the game's window. Reports handles starting at `0x0001_0100`. Registers class if not already, centers the window, and records it. |
| `DefWindowProcA` | `(hWnd: u32, uMsg: u32, wParam: u32, lParam: u32) -> u32` | No-op; returns 0. |
| `DispatchMessageA` | `(lpMsg: *const MSG) -> u32` | Dispatches to the guest window procedure via `guest_call`. Returns 0. |
| `FindWindowA` | `(lpClassName: *const i8, lpWindowName: *const i8) -> u32` | Always returns 0 — only one instance runs in this host. |
| `GetAsyncKeyState` | `(vKey: u32) -> u32` | Returns bit 15 set if the key is currently down, bit 0 set if it was pressed since last call (Windows convention). |
| `GetCursorPos` | `(lpPoint: u32) -> u32` | Writes the cursor x and y coordinates to the guest‑provided buffer. |
| `GetDC` | `(hDC: u32) -> u32` | Returns a device-context handle; each call gets a new DC number starting at `0x0001_0200`. |
| `GetDesktopWindow` | `() -> u32` | Returns `0x0001_0000`, the "desktop" handle. |
| `GetMenu` | `(hMenu: u32) -> u32` | Always returns 0 — the image carries no menu resource. |
| `GetMessageA` | `(lpMsg: *const MSG, hwnd: u32, wFilterMin: u32, wFilterMax: u32) -> u32` | Returns 1 and fills the message structure, or returns 0 with `WM_NULL` when the queue is empty. Designed so the guest keeps running instead of exiting on a missing `WM_QUIT`. |
| `GetWindowDC` | `(hWnd: u32) -> u32` | Same as `GetDC` (no separate implementation). |
| `GetWindowLongA` | `(hWnd: u32, nIndex: i32) -> i32` | Returns stored style/extra data; `GWL_STYLE` reads/writes the window style, `GWL_EXSTYLE` returns 0, others return per-window stored values. |
| `GetWindowRect` | `(hWnd: u32, lpRect: *const i32) -> BOOL` | Writes the window's rect `[left, top, right, bottom]` to the guest buffer. The desktop is `0, 0, 640, 480`. |
| `LoadCursorA` | `(hInstance: u32, lpCursorName: *const i8) -> u32` | Returns a predefined cursor handle (arrow = `0x8000_0020`). |
| `LoadIconA` | `(hInstance: u32, lpIconName: *const i8) -> u32` | Returns a predefined icon handle. |
| `MessageBoxA` | `(hWnd: u32, lpText: *const i8, lpCaption: *const i8, uType: u32) -> u32` | Records the caption and text as a host notice and returns `IDOK` (1). |
| `PeekMessageA` | `(lpMsg: *const MSG, hwnd: u32, wFilterMin: u32, wFilterMax: u32, wRemoveFlags: u32) -> u32` | Returns 1 if a message was found (and copies it), 0 if the queue was empty. When `PM_REMOVE` is set the message is taken; otherwise it stays queued. |
| `PostQuitMessage` | `(nExitCode: u32) -> void` | Posts a notice and returns `Action::Exit(code)`. |
| `RegisterClassA` | `(lpWndClass: *const u8) -> ATOM` | Registers a window class; the "atom" is just a stable non‑zero index. |
| `ReleaseDC` | `(hWnd: u32, hDC: u32) -> i32` | No-op; returns 1. |
| `SetCursorPos` | `(X: i32, Y: i32) -> BOOL` | Moves the guest cursor and the page’s single shared cursor to `(X, Y)`. |
| `SetRect` | `(lpRect: *const i32, left: i32, top: i32, right: i32, bottom: i32) -> BOOL` | Writes four signed dwords into the guest buffer. |
| `SetWindowLongA` | `(hWnd: u32, nIndex: i32, dwNewLong: u32) -> i32` | Stores the new value and returns the previous one. `GWL_STYLE` updates the window style; `GWL_EXSTYLE` is a no-op (returns 0); others are stored per‑window. |
| `SetWindowPos` | `(hWnd: u32, hWndInsertAfter: u32, X: i32, Y: i32, cx: i32, cy: i32, uFlags: u32) -> BOOL` | Updates the window rect. |
| `ShowCursor` | `(bShow: i32) -> i32` | Maintains a display counter; `FALSE` hides the page cursor, `TRUE` shows it. Returns the counter value. |
| `ShowWindow` | `(hWnd: u32, nCmdShow: u32) -> BOOL` | Always returns 1; marks the window as activated and posts `WM_ACTIVATE`, `WM_SETFOCUS`, `WM_ACTIVATEAPP`. |
| `SystemParametersInfoA` | `(uAction: u32, uParam: u32, lpvParam: *const u8, fuWinIni: u32) -> BOOL` | Answers `SPI_GETWORKAREA` with `[0, 0, 640, 480]`; everything else returns 0 and sets last error to 87 (ERROR_INVALID_PARAMETER). |
| `TranslateMessage` | `(lpMsg: *const MSG) -> u32` | Generates a `WM_CHAR` from a `WM_KEYDOWN`; the character is the input‑layer’s translation of the key. |
| `UpdateWindow` | `(hWnd: u32) -> BOOL` | No-op; returns 1. |

---

## kernel32.rs — KERNEL32.dll subset

Provides process management, heap/ memory operations, environment strings, and
version queries. Text is Latin‑1 end to end; no real code‑page tables.

**Functions:**

| Function | Args | Description |
|---|---|---|
| `Beep` | `(dwFreq: u32, dwDuration: u32) -> BOOL` | No‑op; returns 1. |
| `CloseHandle` | `(hObject: u32) -> BOOL` | No‑op; returns 1. |
| `CompareStringA` / `CompareStringW` | `(lpLocaleName: u32, dwFlags: u32, lpString1: *const i8, cbString1: i32, lpString2: *const i8, cbString2: i32, lpResult: *const u32) -> u32` | Not implemented; returns 0. |
| `ExitProcess` | `(uExitCode: u32) -> !` | Exits the host process with the given code. |
| `FileTimeToLocalFileTime` / `FileTimeToSystemTime` | `(lpFileTime: *const u64, lpLocalFileTime: *const u64)` / `(lpFileTime: *const u64, lpSystemTime: *const u32) -> BOOL` | Not implemented; returns 0. |
| `FreeEnvironmentStringsA` / `W` | `() -> u32` | No‑op; returns 1. |
| `GetACP` | `() -> u32` | Returns the ANSI code page; default 0. |
| `GetCPInfo` | `(uCodePage: u32, lpCPInfo: *const u32) -> BOOL` | Not implemented; returns 0. |
| `GetCommandLineA` | `() -> *const i8` | Returns a pointer to the command line string (embedded in the host). |
| `GetCurrentProcess` | `() -> u32` | Returns `INVALID_HANDLE_VALUE` (0xffff_ffff). |
| `GetEnvironmentStringsA` / `W` | `() -> *const i8` | Returns the environment block (empty/absent). |
| `GetFileType` | `(hFile: u32) -> u32` | Returns `FILE_TYPE_CHAR` (2) for stdin/stdout/stderr handles, otherwise 0 (or the file’s type from the file table). |
| `GetLastError` | `() -> u32` | Returns the host‑last‑error value (set on purpose by some shims). |
| `GetLocalTime` | `(lpSystemTime: *const u32) -> BOOL` | Not implemented; returns 0. |
| `GetLogicalDrives` | `() -> u32` | Returns 1 (only the RAM drive is available). |
| `GetModuleFileNameA` | `(hModule: u32, lpFilename: u32, nSize: u32) -> u32` | Writes `Doraemon.exe` into the buffer. |
| `GetModuleHandleA` | `(lpModuleName: *const i8) -> u32` | Returns `IMAGE_BASE` (the game’s base address). |
| `GetOEMCP` | `() -> u32` | Returns 0 (no OEM code page support). |
| `GetProfileStringA` | `(lpAppName: *const i8, lpKeyName: *const i8, lpDefault: *const i8, lpBuffer: u32, nSize: u32) -> u32` | Always writes the default string and returns its length; used for INI‑like queries. |
| `GetProcAddress` | `(hModule: u32, lpProcName: *const i8) -> u32` | Not implemented; returns 0. |
| `GetStartupInfoA` | `(lpStartupInfo: u32) -> BOOL` | Zeroes a STARTUPINFOA structure (68 bytes) and writes its size. |
| `GetVersion` | `() -> u32` | Returns `0x0a28_0105` (Windows XP 5.1) for version‑check compatibility. |
| `GlobalLock` / `GlobalUnlock` | `(hMem: u32) -> u32` / `() -> BOOL` | Identity: returns the handle / 1. The game treats every handle as a fixed pointer. |
| `HeapAlloc` | `(hHeap: u32, dwFlags: u32, dwBytes: u32) -> u32` | Allocates from the bump‑with‑reuse arena. Reuses freed blocks best‑fit; grows the arena if the request is larger than remaining space. |
| `HeapCreate` | `(flOptions: u32, reserved: u32, reserved2: u32) -> u32` | Creates a heap handle from the single arena; the handle is just a distinct non‑zero number. |
| `HeapDestroy` | `(hHeap: u32) -> BOOL` | No‑op; returns 1. |
| `HeapFree` | `(hHeap: u32, dwFlags: u32, lpMem: *const u8) -> BOOL` | Releases a block back to the free list for best‑fit reuse. |
| `HeapReAlloc` | `(hHeap: u32, dwFlags: u32, lpMem: *const u8, dwBytes: u32) -> u32` | Reallocates a block in the arena; the old block stays in place if the move fails. |
| `IsBadCodePtr` / `IsBadReadPtr` / `IsBadWritePtr` | `(addr: u32) -> BOOL` | Always returns 0 — the memory layer checks every access anyway. |
| `LCMapStringA` / `W` | `(lpLocaleName: u32, dwFlags: u32, lpSrcStr: *const i8, cchSrc: i32, lpDstStr: u32, cchDst: i32) -> u32` | Not implemented; returns 0. |
| `LoadLibraryA` | `(lpFileName: *const i8) -> u32` | No‑op; returns 0 and sets last error to 126 (ERROR_MOD_NOT_FOUND). |
| `MultiByteToWideChar` | `(dwFlags: u32, lpMultiByteStr: *const i8, cchMultiByte: i32, lpWideCharStr: u32, cchWideChar: i32) -> u32` | Maps bytes to code points one‑for‑one (Latin‑1 round‑trip). |
| `RaiseException` | `(dwExceptionCode: u32, dwFlags: u32, nNumberOfArguments: u32, lpArguments: *const u32) -> !` | Returns an error that the session catches; the guest’s SEH path runs. |
| `RtlUnwind` | `(ExceptionFrame: u32, TargetIp: u32, ContextRecord: u32, Handler: u32) -> !` | No‑op; returns 0. |
| `SetEnvironmentVariableA` | `(lpName: *const i8, lpValue: *const i8) -> BOOL` | No‑op; returns 1. |
| `SetHandleCount` | `(uHandleCount: u32) -> u32` | Returns the count argument unchanged. |
| `SetStdHandle` | `(nStdHandle: i32, hHandle: u32) -> BOOL` | No‑op; returns 1. |
| `Sleep` | `(dwMilliseconds: u32) -> BOOL` | No‑op; returns 0. The frame clock owns pacing; a blocking sleep would freeze the page. |
| `TerminateProcess` | `(uExitCode: u32, dwExitCode: u32) -> !` | Exits the host process. |
| `UnhandledExceptionFilter` | `(lpExceptionPointers: *const u32) -> u32` | Returns 1 (the CRT installs its own filter). |
| `VirtualAlloc` | `(lpAddress: u32, dwSize: u32, flAllocationType: u32, flProtect: u32) -> u32` | Allocates from the arena. If `lpAddress` is 0 the best‑fit algorithm is used; otherwise allocation at the exact address (or failure). |
| `VirtualFree` | `(lpAddress: u32, dwSize: u32, dwFreeType: u32) -> BOOL` | No‑op; releases the block back to the arena’s free list. |
| `WideCharToMultiByte` | `(dwFlags: u32, lpWideCharStr: *const i8, cchWideChar: i32, lpMultiByteStr: u32, cchMultiByte: i32) -> u32` | Reverse of `MultiByteToWideChar`; Latin‑1 one‑for‑one. |
| `lstrcmpiA` | `(lpString1: *const i8, lpString2: *const i8) -> i32` | Case‑insensitive comparison; returns 0 if equal, sign of difference otherwise. |
| `lstrcpyA` | `(lpDest: u32, lpSrc: *const i8) -> u32` | Copies a NUL‑terminated string into the guest buffer; returns the length. |

---

## ddraw.rs — DirectDraw.dll subset

This is the core graphics shim. It implements the bare minimum for the game to
run: a primary surface with a palette, flips, and palette entry writes. Every
fade in this game is a palette‑only change (no flip), and the host now tracks
that so the page repaints correctly.

**Structures and methods on `DirectDraw`:**

| Method | Signature | Description |
|---|---|---|
| `default()` | `Self` | Creates a default instance with 640×480, 256‑entry palette (all black), flip counter 0, palette‑revision counter 0. |
| `width()` | `-> u32` | Returns 640. |
| `height()` | `-> u32` | Returns 480. |
| `palette()` | `-> &[[u8; 3]; 256]` | The current palette; each entry is `[R, G, B]`. |
| `set_entry(index: usize, colour: [u8; 3])` | `()` | Writes a single palette entry; bumps `palette_revision` only if the new colour differs from the old one. |
| `reserve_ends()` | `()` | Internal: reserves the first and last palette entries (0 and 255) for the host’s use; entry 0 is always `[0,0,0]`, entry 255 is `[255,255,255]`. |
| `flips()` | `-> u64` | Total number of `IDirectDrawSurface7::Flip` calls. |
| `palette_revision()` | `-> u32` | The number of times a palette entry actually changed (colour differed from before). |
| `framebuffer()` | `-> Option<(u32, u32, u32)>` | `(address, width, height)` of the primary surface framebuffer, or `None` if not initialised. |
| `framebuffer_pitch()` | `-> u32` | Stride in bytes: `(width * 3 + 3) & ~3`. |
| `surface_count()` | `-> usize` | Number of surfaces created (normally 1 for the primary). |

**Exported shim bindings (from `SHIMS`):**

| Binding | Args | Description |
|---|---|---|
| `SetEntry` | `(index: u32, colour: [u8; 3]) -> u32` | Writes a palette entry; returns the previous colour (as a packed u32). The host records the change only if the colour actually differs — `palette_revision` increments only then. |
| `DirectDrawCreate` | `(lpGUID: *const u8, lplpDD: u32, lpDDOrigin: *const u8) -> HRESULT` | Returns a `DirectDraw` object (handle `1`). |
| `GetPalette` | `(lpDDPalette: u32, lpPalette: u32) -> HRESULT` | Returns the current palette pointer. |
| `SetPalette` | `(lpPalette: u32, iStart: u32, cEntries: u32, lpEntries: *const u32) -> HRESULT` | Writes multiple palette entries starting at `iStart` for `cEntries` count. Bumps `palette_revision` per entry that actually changes colour. |
| `Flip` | `() -> u32` | Presents the current frame; increments `flips()`. |
| `WaitForVerticalBlank` | `(dwFlags: u32) -> HRESULT` | No‑op; returns `DS_OK` (0). |

---

## dsound.rs — DirectSound.dll subset

Provides sound playback, voices, and effects. The game plays PCM samples as
voices; the page mixes them into a single output.

**Key types and methods:**

| Type/Method | Description |
|---|---|
| `DirectSoundCreate` | Creates a `DirectSound` object. |
| `CreateSoundBuffer` | Creates a secondary buffer for a PCM sample. |
| `Play` | Starts playing a voice (one‑shot or looping). |
| `Pause` / `Resume` | Pauses/resumes a playing voice. |
| `Stop` | Stops a voice. |
| `SetVolume` / `SetPan` | Adjusts volume (7 levels, −1000 to 0) and pan (−1000 to 1000). |
| `GetVolume` / `GetPan` | Reads the current settings. |
| `StatusOf` / `PositionOf` | Reads the playhead and elapsed bytes. |
| `Lock` / `Unlock` | Locks a voice’s buffer so the host can fill it with PCM data; unlock publishes the new samples. |

---

## winmm.rs — WinMM.dll subset

Provides timers and joystick support. The game uses `timeSetEvent` for its
frame pacing and `mciSendCommand` for resource loading.

**Functions:**

| Function | Args | Description |
|---|---|---|
| `timeBeginPeriod` | `(uPeriod: u32) -> u32` | No‑op; returns 1. Adjusts the system timer resolution. |
| `timeEndPeriod` | `(uPeriod: u32) -> u32` | No‑op; returns 1. |
| `timeGetDevCaps` | `(lpCaps: u32, uSize: u32) -> u32` | Returns capability bits; the timer resolution is always 1 ms. |
| `timeSetEvent` | `(uDelayMs: u32, uResolution: u32, lpTimerProc: u32, dwUser: u32, fuFlags: u32) -> u32` | Creates a one‑shot timer that fires once after `uDelayMs` milliseconds. The callback is stored but never called (the page’s frame loop is the pacing mechanism). |
| `timeKillEvent` | `(uTimerID: u32) -> BOOL` | No‑op; returns 1. |
| `joystick_unplugged` | `() -> BOOL` | No‑op; returns 1. |
| `mciSendCommandA` / `W` | `(uDeviceID: u32, uMsg: u32, fdwCmd: u32, dwParam: u32) -> u32` | Not fully implemented; returns 0 for most commands. |

---

## dinput.rs — DirectInput.dll subset

Provides keyboard and mouse input. The game reads key states and mouse
coordinates; the host drives a shared cursor and queue of messages.

**Key functions:**

| Function | Args | Description |
|---|---|---|
| `DirectInputCreate` | Creates a `DirectInput` object. |
| `CreateDevice` | Creates a device (keyboard or mouse). |
| `SetCooperativeLevel` | Sets the cooperative level window. |
| `SetDataFormat` | Sets the data format for the device. |
| `Acquire` / `Unacquire` | Acquires/relinquishes the device. |
| `GetDeviceState` | Reads the current state of the device. |
| `CreateEffect` | Creates an effect (not used by this game). |

The host maintains a single shared cursor position and a message queue that
`PeekMessageA`/`GetMessageA` read from.

---

## advapi32.rs — Advapi32.dll subset

Registry operations for the game’s INI‑style configuration. The game reads and
writes keys under `HKEY_CURRENT_USER\Software\Doramon`.

**Functions:**

| Function | Args | Description |
|---|---|---|
| `reg_open_key_ex` | Opens or creates a registry key. |
| `reg_close_key` | Closes a previously opened key. |
| `reg_query_value_ex` | Queries a value from an open key; returns the size it needs (or 0 if missing). |
| `reg_set_value_ex` | Sets a value under an open key. |
| `optional_cstr` | Helper: reads a C string from the guest memory, or returns "" if the pointer is 0. |
| `open_game_key` | Opens the install key `Software\Doramon` (creates it if missing). |
| `a_value_round_trips_and_reports_the_size_it_needs` | Queries a value, gets its size, reports it back. |
| `a_missing_value_is_file_not_found` | Queries a missing value; reports error 127 (PROC_NOT_FOUND) rather than writing zeroes. |
| `a_closed_handle_is_no_longer_usable` | After a key is closed, subsequent queries report the key is gone. |
| `a_closed_handle_is_no_longer_usable` | Mirror of the above for read‑only handles. |

---

## files.rs — Files subsystem

The game opens files by pathname; the host serves them from the bundled ROM and
a small writable area for saves.

**Key functions:**

| Function | Args | Description |
|---|---|---|
| `create_file` | `(lpFileName: u32, dwDesiredAccess: u32, lpSecurityAttributes: u32, lpCreationDistribution: u32, lpFileTemplate: u32) -> u32` | Opens a read‑only bundle file by basename; returns a handle or 0 (file-not-found). |
| `write_file` | `(hFile: u32, lpBuffer: u32, nNumberOfBytesToWrite: u32) -> u32` | Writes bytes into the saves area; returns the number written. |
| `set_end_of_file` | `(hFile: u32, dwSize: u32) -> BOOL` | No‑op; returns 1. |
| `flush_file_buffers` | `(hFile: u32) -> BOOL` | No‑op; returns 1. |
| `delete_file` | `(lpFileName: u32) -> BOOL` | No‑op; returns 1 (the bundle is read‑only). |
| `find_first_file` | `(lpFileName: u32, lpFindFileData: u32) -> u32` | Starts a search; returns a search handle or 0. |
| `find_close` | `(hFindFile: u32) -> BOOL` | Ends a search handle. |
| `read_file` | `(hFile: u32, lpBuffer: u32, nNumberOfBytesToRead: u32, lpNumberOfBytesRead: u32) -> u32` | Reads bytes from a bundle file or the saves area. |
| `set_file_pointer` | `(hFile: u32, lDistanceToMove: i32, lpFileDistance: *const u32) -> u32` | Seeks within a file; returns the new position. |
| `close` | `(handle: u32) -> bool` | Removes the handle from the table. |
| `file_type` | `(handle: u32) -> Option<u32>` | Returns `FILE_TYPE_CHAR` for standard streams, or the file’s type from the table. |
| `exists` | `(path: &str) -> bool` | Checks the bundle or saves area. |
| `is_open` | `(handle: u32) -> bool` | Checks if a handle is tracked. |
| `saved` | `(path: &str) -> bool` | Checks the saves directory. |
| `create` | `(path: &str, creation: u32) -> Option<u32>` | Creates a file in the saves area; returns a handle or `None`. |
| `write` | `(handle: u32, bytes: &[u8]) -> Option<u32>` | Writes bytes to an open save‑file handle. |
| `end_of_file` | `(handle: u32) -> bool` | Checks if the file cursor is at the end. |
| `delete` | `(path: &str) -> bool` | Deletes a file from the saves area. |
| `version` | `() -> u64` | Returns the bundle’s overall version (hash of all bundle files). |
| `find` | `(pattern: &str) -> Option<(String, u32)>` | Finds a file matching a glob pattern in the bundle; returns (basename, size). |
| `open_search` | `() -> u32` | Starts a bundle‑only search (no saves). |
| `close_search` | `(handle: u32) -> bool` | Closes a search handle from `open_search`. |
| `wildcard` | `(pattern: &str, name: &str) -> bool` | Checks if a name matches a shell‑style glob. |
| `normalize` | `(path: &str) -> String` | Normalises a path (lower‑case, collapse `//`, etc.). |
| `basename` | `(path: &str) -> String` | Extracts the final component of a path. |

---

## License

Internal rehost project — see the repository root for licensing terms.