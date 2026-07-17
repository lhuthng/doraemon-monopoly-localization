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
    DORA_AUDIO_INIT = 0,
    DORA_AUDIO_PLAY = 1,
    DORA_AUDIO_STOP = 2,
    DORA_AUDIO_DURATION = 3,
    DORA_AUDIO_COUNT = 4,
    DORA_AUDIO_VOLUME = 5,
    DORA_AUDIO_CLOSE = 6,
    DORA_AUDIO_GET_VOLUME = 7
};

typedef struct {
    uint32_t id;
    uint32_t offset;
    uint32_t encoded_length;
    uint32_t frames;
} TrackEntry;

static HANDLE music_file = INVALID_HANDLE_VALUE;
static HANDLE worker_thread;
static volatile LONG worker_alive;
static CRITICAL_SECTION audio_lock;
static int lock_ready;
static IDirectSound *direct_sound;
static IDirectSoundBuffer *music_buffer;
static TrackEntry tracks[DMUS_TRACKS];
static uint32_t encoded_remaining;
static uint32_t active_offset;
static uint32_t active_length;
static uint32_t block_remaining;
static int16_t predictor_l;
static int16_t predictor_r;
static int index_l;
static int index_r;
static int block_first;
static int playing;
static int play_chunk;
static LONG current_volume;
static DWORD current_level = 65535;

static void beside_executable(char *path, const char *name) {
    DWORD length = GetModuleFileNameA(NULL, path, MAX_PATH);
    if (!length || length >= MAX_PATH) {
        path[0] = 0;
        return;
    }
    while (length && path[length - 1] != '\\' && path[length - 1] != '/') --length;
    lstrcpyA(path + length, name);
}

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

static void log_value(const char *prefix, DWORD value) {
    char line[64];
    DWORD length = 0;
    while (prefix[length] && length < 50) {
        line[length] = prefix[length];
        ++length;
    }
    for (int shift = 28; shift >= 0; shift -= 4) {
        DWORD digit = (value >> shift) & 15;
        line[length++] = (char)(digit < 10 ? '0' + digit : 'A' + digit - 10);
    }
    line[length] = 0;
    log_text(line);
}

static const int step_table[89] = {
    7,8,9,10,11,12,13,14,16,17,19,21,23,25,28,31,34,37,41,45,50,55,60,66,
    73,80,88,97,107,118,130,143,157,173,190,209,230,253,279,307,337,371,
    408,449,494,544,598,658,724,796,876,963,1060,1166,1282,1411,1552,1707,
    1878,2066,2272,2499,2749,3024,3327,3660,4026,4428,4871,5358,5894,6484,
    7132,7845,8630,9493,10442,11487,12635,13899,15289,16818,18500,20350,
    22385,24623,27086,29794,32767
};
static const int index_table[16] = {
    -1,-1,-1,-1,2,4,6,8,-1,-1,-1,-1,2,4,6,8
};
static const LONG volume_table[54] = {
    -10000,-3449,-2847,-2495,-2245,-2051,-1893,-1759,-1643,-1541,-1450,
    -1367,-1291,-1221,-1156,-1095,-1038,-985,-934,-886,-840,-796,-754,
    -713,-674,-636,-600,-565,-531,-498,-466,-435,-405,-376,-347,-319,
    -292,-265,-239,-214,-189,-165,-142,-118,-95,-73,-51,-30,-8,0,0,0,0,0
};

static int read_exact(void *target, DWORD length) {
    DWORD read = 0;
    return ReadFile(music_file, target, length, &read, NULL) && read == length;
}

static int16_t decode_nibble(int nibble, int16_t *predictor, int *index) {
    int step = step_table[*index];
    int difference = step >> 3;
    if (nibble & 1) difference += step >> 2;
    if (nibble & 2) difference += step >> 1;
    if (nibble & 4) difference += step;
    int sample = *predictor + ((nibble & 8) ? -difference : difference);
    if (sample > 32767) sample = 32767;
    if (sample < -32768) sample = -32768;
    *index += index_table[nibble & 15];
    if (*index < 0) *index = 0;
    if (*index > 88) *index = 88;
    *predictor = (int16_t)sample;
    return *predictor;
}

static int begin_block(void) {
    unsigned char header[10];
    if (encoded_remaining < sizeof(header) && active_length) {
        SetFilePointer(music_file, (LONG)active_offset, NULL, FILE_BEGIN);
        encoded_remaining = active_length;
    }
    if (encoded_remaining < sizeof(header) || !read_exact(header, sizeof(header))) return 0;
    encoded_remaining -= sizeof(header);
    block_remaining = (uint32_t)header[0] | ((uint32_t)header[1] << 8);
    predictor_l = (int16_t)((uint16_t)header[2] | ((uint16_t)header[3] << 8));
    index_l = header[4];
    predictor_r = (int16_t)((uint16_t)header[6] | ((uint16_t)header[7] << 8));
    index_r = header[8];
    if (!block_remaining || block_remaining > 4096 || index_l > 88 || index_r > 88) return 0;
    block_first = 1;
    return 1;
}

static void decode_frames(int16_t *output, DWORD frames) {
    while (frames--) {
        if (!block_remaining && !begin_block()) {
            *output++ = 0;
            *output++ = 0;
            continue;
        }
        if (block_first) {
            *output++ = predictor_l;
            *output++ = predictor_r;
            block_first = 0;
        } else {
            unsigned char packed = 0;
            if (!encoded_remaining || !read_exact(&packed, 1)) {
                encoded_remaining = 0;
                block_remaining = 0;
                *output++ = 0;
                *output++ = 0;
                continue;
            }
            --encoded_remaining;
            *output++ = decode_nibble(packed & 15, &predictor_l, &index_l);
            *output++ = decode_nibble(packed >> 4, &predictor_r, &index_r);
        }
        --block_remaining;
    }
}

static int fill_region(DWORD offset, DWORD length) {
    void *first = NULL, *second = NULL;
    DWORD first_length = 0, second_length = 0;
    HRESULT result = IDirectSoundBuffer_Lock(
        music_buffer, offset, length, &first, &first_length, &second, &second_length, 0);
    if (result == DSERR_BUFFERLOST) {
        IDirectSoundBuffer_Restore(music_buffer);
        result = IDirectSoundBuffer_Lock(
            music_buffer, offset, length, &first, &first_length, &second, &second_length, 0);
    }
    if (FAILED(result)) {
        log_value("BUFFER_LOCK_FAIL 0x", (DWORD)result);
        return 0;
    }
    decode_frames((int16_t *)first, first_length / PCM_FRAME_BYTES);
    if (second_length) decode_frames((int16_t *)second, second_length / PCM_FRAME_BYTES);
    IDirectSoundBuffer_Unlock(music_buffer, first, first_length, second, second_length);
    return 1;
}

static DWORD WINAPI stream_worker(void *unused) {
    (void)unused;
    while (InterlockedCompareExchange(&worker_alive, 1, 1)) {
        Sleep(20);
        EnterCriticalSection(&audio_lock);
        if (playing && music_buffer) {
            DWORD cursor = 0;
            if (SUCCEEDED(IDirectSoundBuffer_GetCurrentPosition(music_buffer, &cursor, NULL))) {
                int current = (int)(cursor / CHUNK_BYTES);
                while (play_chunk != current) {
                    fill_region((DWORD)play_chunk * CHUNK_BYTES, CHUNK_BYTES);
                    play_chunk = (play_chunk + 1) & 3;
                }
            }
        }
        LeaveCriticalSection(&audio_lock);
    }
    return 0;
}

static int create_buffer(void) {
    if (music_buffer) return 1;
    if (!direct_sound) return 0;
    WAVEFORMATEX format;
    DSBUFFERDESC descriptor;
    unsigned char *clear = (unsigned char *)&format;
    for (unsigned int i = 0; i < sizeof(format); ++i) clear[i] = 0;
    clear = (unsigned char *)&descriptor;
    for (unsigned int i = 0; i < sizeof(descriptor); ++i) clear[i] = 0;
    format.wFormatTag = WAVE_FORMAT_PCM;
    format.nChannels = PCM_CHANNELS;
    format.nSamplesPerSec = PCM_RATE;
    format.nAvgBytesPerSec = PCM_RATE * PCM_FRAME_BYTES;
    format.nBlockAlign = PCM_FRAME_BYTES;
    format.wBitsPerSample = 16;
    descriptor.dwSize = sizeof(descriptor);
    descriptor.dwFlags = DSBCAPS_CTRLVOLUME | DSBCAPS_GETCURRENTPOSITION2 | DSBCAPS_GLOBALFOCUS;
    descriptor.dwBufferBytes = BUFFER_BYTES;
    descriptor.lpwfxFormat = &format;
    HRESULT result = IDirectSound_CreateSoundBuffer(direct_sound, &descriptor, &music_buffer, NULL);
    if (FAILED(result)) {
        descriptor.dwFlags = DSBCAPS_CTRLVOLUME | DSBCAPS_GLOBALFOCUS;
        result = IDirectSound_CreateSoundBuffer(direct_sound, &descriptor, &music_buffer, NULL);
    }
    if (FAILED(result)) {
        descriptor.dwFlags = DSBCAPS_CTRLVOLUME;
        result = IDirectSound_CreateSoundBuffer(direct_sound, &descriptor, &music_buffer, NULL);
    }
    if (FAILED(result)) {
        log_value("BUFFER_CREATE_FAIL 0x", (DWORD)result);
        return 0;
    }
    log_text("BUFFER_CREATE_OK");
    IDirectSoundBuffer_SetVolume(music_buffer, current_volume);
    return 1;
}

static int open_music(uintptr_t manager) {
    if (music_file != INVALID_HANDLE_VALUE) return 1;
    char path[MAX_PATH];
    log_text("INIT_BEGIN");
    beside_executable(path, "Music.dat");
    if (!path[0]) {
        log_text("INIT_PATH_FAIL");
        return 0;
    }
    music_file = CreateFileA(path, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    if (music_file == INVALID_HANDLE_VALUE) {
        log_text("INIT_OPEN_FAIL");
        return 0;
    }
    unsigned char header[DMUS_HEADER_SIZE];
    if (!read_exact(header, sizeof(header)) ||
        header[0] != 'D' || header[1] != 'M' || header[2] != 'U' || header[3] != 'S' ||
        header[4] != 'I' || header[5] != 'C' || header[6] != '1' || header[7] != 0 ||
        *(uint32_t *)(header + 8) != 1 || *(uint32_t *)(header + 12) != DMUS_TRACKS ||
        *(uint32_t *)(header + 16) != PCM_RATE || *(uint16_t *)(header + 20) != PCM_CHANNELS ||
        *(uint16_t *)(header + 22) != 16 || *(uint32_t *)(header + 24) != 4096) {
        CloseHandle(music_file);
        music_file = INVALID_HANDLE_VALUE;
        log_text("INIT_HEADER_FAIL");
        return 0;
    }
    for (int i = 0; i < DMUS_TRACKS; ++i) {
        unsigned char *entry = header + 32 + i * 16;
        tracks[i].id = *(uint32_t *)(entry + 0);
        tracks[i].offset = *(uint32_t *)(entry + 4);
        tracks[i].encoded_length = *(uint32_t *)(entry + 8);
        tracks[i].frames = *(uint32_t *)(entry + 12);
        if (tracks[i].id != (uint32_t)(i + 2) || tracks[i].offset < DMUS_HEADER_SIZE || !tracks[i].encoded_length) {
            CloseHandle(music_file);
            music_file = INVALID_HANDLE_VALUE;
            log_text("INIT_DIRECTORY_FAIL");
            return 0;
        }
    }
    if (manager) direct_sound = *(IDirectSound **)(manager + 0x10c);
    if (!lock_ready) {
        InitializeCriticalSection(&audio_lock);
        lock_ready = 1;
        worker_alive = 1;
        worker_thread = CreateThread(NULL, 0, stream_worker, NULL, 0, NULL);
        if (!worker_thread) worker_alive = 0;
    }
    if (!direct_sound) log_text("INIT_NO_DIRECTSOUND");
    if (!worker_thread) log_text("INIT_NO_WORKER");
    if (direct_sound && worker_thread) log_text("INIT_OK");
    return direct_sound && worker_thread;
}

static int play_track(uint32_t id) {
    log_value("PLAY_TRACK 0x", id);
    if (id < 2 || id > 11) {
        log_text("PLAY_BAD_TRACK");
        return 0;
    }
    if (music_file == INVALID_HANDLE_VALUE) {
        log_text("PLAY_NO_FILE");
        return 0;
    }
    if (!create_buffer()) return 0;
    TrackEntry *track = &tracks[id - 2];
    EnterCriticalSection(&audio_lock);
    IDirectSoundBuffer_Stop(music_buffer);
    SetFilePointer(music_file, (LONG)track->offset, NULL, FILE_BEGIN);
    active_offset = track->offset;
    active_length = track->encoded_length;
    encoded_remaining = track->encoded_length;
    block_remaining = 0;
    block_first = 0;
    if (!fill_region(0, BUFFER_BYTES)) {
        LeaveCriticalSection(&audio_lock);
        return 0;
    }
    IDirectSoundBuffer_SetCurrentPosition(music_buffer, 0);
    IDirectSoundBuffer_SetVolume(music_buffer, current_volume);
    play_chunk = 0;
    HRESULT result = IDirectSoundBuffer_Play(music_buffer, 0, 0, DSBPLAY_LOOPING);
    playing = SUCCEEDED(result);
    if (playing) log_text("PLAY_OK"); else log_value("PLAY_FAIL 0x", (DWORD)result);
    LeaveCriticalSection(&audio_lock);
    return playing;
}

static void stop_music(void) {
    if (!lock_ready) return;
    EnterCriticalSection(&audio_lock);
    playing = 0;
    if (music_buffer) IDirectSoundBuffer_Stop(music_buffer);
    LeaveCriticalSection(&audio_lock);
}

static void close_music(void) {
    stop_music();
    if (worker_thread) {
        InterlockedExchange(&worker_alive, 0);
        WaitForSingleObject(worker_thread, 2000);
        CloseHandle(worker_thread);
        worker_thread = NULL;
    }
    if (music_buffer) {
        IDirectSoundBuffer_Release(music_buffer);
        music_buffer = NULL;
    }
    if (music_file != INVALID_HANDLE_VALUE) {
        CloseHandle(music_file);
        music_file = INVALID_HANDLE_VALUE;
    }
    if (lock_ready) {
        DeleteCriticalSection(&audio_lock);
        lock_ready = 0;
    }
}

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
            } else {
                log_text("VOLUME_NO_BUFFER");
            }
            return 1;
        }
        case DORA_AUDIO_CLOSE: close_music(); return 1;
        case DORA_AUDIO_GET_VOLUME: return current_level;
        default: (void)second; return 0;
    }
}

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
    (void)instance;
    (void)reserved;
    /* Never wait for the worker while the Windows loader lock is held. The
       game can call command 6 for an orderly close; process teardown reclaims
       the remaining handles and DirectSound objects automatically. */
    if (reason == DLL_PROCESS_DETACH) InterlockedExchange(&worker_alive, 0);
    return TRUE;
}
