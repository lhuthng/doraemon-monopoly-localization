/*
 * Doraemon local-music bridge.
 *
 * This file exposes the tiny DoraAudioDispatch ABI called by the patched
 * game. The game expects CD/MCI audio; the bridge supplies deterministic local
 * Music.dat playback through DirectSound. Shared state and command routing
 * remain here, while the ADPCM decoder and DirectSound lifecycle are kept in
 * doraudio_codec.c and doraudio_stream.c. They are included below so the
 * existing one-object MinGW build stays simple.
 */
#define COBJMACROS
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <mmsystem.h>
#include <dsound.h>
#include <stdint.h>

#define DMUS_TRACKS 10
#define DMUS_HEADER_SIZE 192
#define PCM_RATE 44100
#define PCM_CHANNELS 2
#define PCM_FRAME_BYTES 4
#define CHUNK_BYTES (PCM_RATE * PCM_FRAME_BYTES)
#define BUFFER_BYTES (CHUNK_BYTES * 4)

enum {
    DORA_AUDIO_INIT = 0, DORA_AUDIO_PLAY = 1, DORA_AUDIO_STOP = 2,
    DORA_AUDIO_DURATION = 3, DORA_AUDIO_COUNT = 4, DORA_AUDIO_VOLUME = 5,
    DORA_AUDIO_CLOSE = 6, DORA_AUDIO_GET_VOLUME = 7
};

typedef struct {
    uint32_t id;
    uint32_t offset;
    uint32_t encoded_length;
    uint32_t frames;
} TrackEntry;

/* Shared state is intentionally private to this translation unit. */
static HANDLE music_file = INVALID_HANDLE_VALUE;
static HANDLE worker_thread;
static volatile LONG worker_alive;
static CRITICAL_SECTION audio_lock;
static int lock_ready;
static IDirectSound *direct_sound;
static IDirectSoundBuffer *music_buffer;
static TrackEntry tracks[DMUS_TRACKS];
static uint32_t encoded_remaining, active_offset, active_length, block_remaining;
static int16_t predictor_l, predictor_r;
static int index_l, index_r, block_first, playing, play_chunk;
static LONG current_volume;
static DWORD current_level = 65535;

/* Finds the executable directory so Music.dat works regardless of CWD. */
static void beside_executable(char *path, const char *name) {
    DWORD length = GetModuleFileNameA(NULL, path, MAX_PATH);
    if (!length || length >= MAX_PATH) { path[0] = 0; return; }
    while (length && path[length - 1] != '\\' && path[length - 1] != '/') --length;
    lstrcpyA(path + length, name);
}

/* Appends a short diagnostic line beside the game without requiring CRT I/O. */
static void log_text(const char *text) {
    char path[MAX_PATH];
    beside_executable(path, "doraudio.log");
    if (!path[0]) return;
    HANDLE file = CreateFileA(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
        NULL, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) return;
    DWORD length = 0, written = 0;
    while (text[length]) ++length;
    WriteFile(file, text, length, &written, NULL);
    WriteFile(file, "\r\n", 2, &written, NULL);
    CloseHandle(file);
}

/* Logs HRESULTs and numeric state in a CRT-free fixed-width hexadecimal form. */
static void log_value(const char *prefix, DWORD value) {
    char line[64];
    DWORD length = 0;
    while (prefix[length] && length < 50) line[length] = prefix[length], ++length;
    for (int shift = 28; shift >= 0; shift -= 4) {
        DWORD digit = (value >> shift) & 15;
        line[length++] = (char)(digit < 10 ? '0' + digit : 'A' + digit - 10);
    }
    line[length] = 0;
    log_text(line);
}

/* IMA-ADPCM step/index tables and the game's 54-position dB curve. */
static const int step_table[89] = {
    7,8,9,10,11,12,13,14,16,17,19,21,23,25,28,31,34,37,41,45,50,55,60,66,
    73,80,88,97,107,118,130,143,157,173,190,209,230,253,279,307,337,371,
    408,449,494,544,598,658,724,796,876,963,1060,1166,1282,1411,1552,1707,
    1878,2066,2272,2499,2749,3024,3327,3660,4026,4428,4871,5358,5894,6484,
    7132,7845,8630,9493,10442,11487,12635,13899,15289,16818,18500,20350,
    22385,24623,27086,29794,32767
};
static const int index_table[16] = {-1,-1,-1,-1,2,4,6,8,-1,-1,-1,-1,2,4,6,8};
static const LONG volume_table[54] = {
    -10000,-3449,-2847,-2495,-2245,-2051,-1893,-1759,-1643,-1541,-1450,
    -1367,-1291,-1221,-1156,-1095,-1038,-985,-934,-886,-840,-796,-754,
    -713,-674,-636,-600,-565,-531,-498,-466,-435,-405,-376,-347,-319,
    -292,-265,-239,-214,-189,-165,-142,-118,-95,-73,-51,-30,-8,0,0,0,0,0
};

#include "doraudio_codec.c"
#include "doraudio_stream.c"

/* Stable ABI entry point: command 5 is the Music slider, command 7 reads it. */
__declspec(dllexport) DWORD __stdcall DoraAudioDispatch(DWORD command, uintptr_t first, uintptr_t second) {
    switch (command) {
        case DORA_AUDIO_INIT: return open_music(first);
        case DORA_AUDIO_PLAY: return play_track((uint32_t)first & 0xffu);
        case DORA_AUDIO_STOP: stop_music(); return 1;
        case DORA_AUDIO_DURATION:
            first &= 0xffu;
            if (first < 2 || first > 11) return 0;
            return tracks[first - 2].frames / PCM_RATE;
        case DORA_AUDIO_COUNT: return 11;
        case DORA_AUDIO_VOLUME: {
            uint32_t index = ((uint32_t)first * 53u) / 65535u;
            current_level = (DWORD)first;
            current_volume = volume_table[index];
            log_value("VOLUME_LEVEL 0x", current_level);
            log_value("VOLUME_DB 0x", (DWORD)current_volume);
            if (music_buffer) {
                HRESULT result = IDirectSoundBuffer_SetVolume(music_buffer, current_volume);
                if (FAILED(result)) log_value("VOLUME_FAIL 0x", (DWORD)result);
                else log_text("VOLUME_OK");
            } else log_text("VOLUME_NO_BUFFER");
            return 1;
        }
        case DORA_AUDIO_CLOSE: close_music(); return 1;
        case DORA_AUDIO_GET_VOLUME: return current_level;
        default: (void)second; return 0;
    }
}

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
    (void)instance; (void)reserved;
    /* Never wait under loader lock; command 6 performs orderly cleanup. */
    if (reason == DLL_PROCESS_DETACH) InterlockedExchange(&worker_alive, 0);
    return TRUE;
}
