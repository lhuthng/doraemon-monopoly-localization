/*
 * DirectSound stream lifecycle.
 *
 * This module opens Music.dat, owns the looping DirectSound buffer, and maps
 * game track and volume commands to that buffer. The original game asks
 * Windows MCI for CD audio, which modern systems and Wine/CrossOver do not
 * provide reliably. A worker thread refills four PCM chunks ahead of the
 * playback cursor, and audio_lock protects all mutable decoder and buffer
 * state.
 *
 * This file is included by doraudio.c after doraudio_codec.c.
 */

/* Locks a circular DirectSound region and decodes PCM into it. */
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

/* Keeps the next circular chunks decoded while the game is running. */
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

/* Creates the PCM buffer, retrying with fewer optional DirectSound flags. */
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

/* Opens and validates the Music.dat directory, then starts the refill thread. */
static int open_music(uintptr_t manager) {
    if (music_file != INVALID_HANDLE_VALUE) return 1;
    char path[MAX_PATH];
    log_text("INIT_BEGIN");
    beside_executable(path, "Music.dat");
    if (!path[0]) { log_text("INIT_PATH_FAIL"); return 0; }
    music_file = CreateFileA(path, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    if (music_file == INVALID_HANDLE_VALUE) { log_text("INIT_OPEN_FAIL"); return 0; }
    unsigned char header[DMUS_HEADER_SIZE];
    if (!read_exact(header, sizeof(header)) ||
        header[0] != 'D' || header[1] != 'M' || header[2] != 'U' || header[3] != 'S' ||
        header[4] != 'I' || header[5] != 'C' || header[6] != '1' || header[7] != 0 ||
        *(uint32_t *)(header + 8) != 1 || *(uint32_t *)(header + 12) != DMUS_TRACKS ||
        *(uint32_t *)(header + 16) != PCM_RATE || *(uint16_t *)(header + 20) != PCM_CHANNELS ||
        *(uint16_t *)(header + 22) != 16 || *(uint32_t *)(header + 24) != 4096) {
        CloseHandle(music_file); music_file = INVALID_HANDLE_VALUE;
        log_text("INIT_HEADER_FAIL"); return 0;
    }
    for (int i = 0; i < DMUS_TRACKS; ++i) {
        unsigned char *entry = header + 32 + i * 16;
        tracks[i].id = *(uint32_t *)(entry + 0);
        tracks[i].offset = *(uint32_t *)(entry + 4);
        tracks[i].encoded_length = *(uint32_t *)(entry + 8);
        tracks[i].frames = *(uint32_t *)(entry + 12);
        if (tracks[i].id != (uint32_t)(i + 2) || tracks[i].offset < DMUS_HEADER_SIZE || !tracks[i].encoded_length) {
            CloseHandle(music_file); music_file = INVALID_HANDLE_VALUE;
            log_text("INIT_DIRECTORY_FAIL"); return 0;
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

/* Resets decoder state and starts one requested game track in a loop. */
static int play_track(uint32_t id) {
    log_value("PLAY_TRACK 0x", id);
    if (id < 2 || id > 11) { log_text("PLAY_BAD_TRACK"); return 0; }
    if (music_file == INVALID_HANDLE_VALUE) { log_text("PLAY_NO_FILE"); return 0; }
    if (!create_buffer()) return 0;
    TrackEntry *track = &tracks[id - 2];
    EnterCriticalSection(&audio_lock);
    IDirectSoundBuffer_Stop(music_buffer);
    SetFilePointer(music_file, (LONG)track->offset, NULL, FILE_BEGIN);
    active_offset = track->offset; active_length = track->encoded_length;
    encoded_remaining = track->encoded_length; block_remaining = 0; block_first = 0;
    if (!fill_region(0, BUFFER_BYTES)) { LeaveCriticalSection(&audio_lock); return 0; }
    IDirectSoundBuffer_SetCurrentPosition(music_buffer, 0);
    IDirectSoundBuffer_SetVolume(music_buffer, current_volume);
    play_chunk = 0;
    HRESULT result = IDirectSoundBuffer_Play(music_buffer, 0, 0, DSBPLAY_LOOPING);
    playing = SUCCEEDED(result);
    if (playing) log_text("PLAY_OK"); else log_value("PLAY_FAIL 0x", (DWORD)result);
    LeaveCriticalSection(&audio_lock);
    return playing;
}

/* Stops playback while retaining the decoded file and DirectSound buffer. */
static void stop_music(void) {
    if (!lock_ready) return;
    EnterCriticalSection(&audio_lock);
    playing = 0;
    if (music_buffer) IDirectSoundBuffer_Stop(music_buffer);
    LeaveCriticalSection(&audio_lock);
}

/* Stops the worker and releases every helper-owned Windows resource. */
static void close_music(void) {
    stop_music();
    if (worker_thread) {
        InterlockedExchange(&worker_alive, 0);
        WaitForSingleObject(worker_thread, 2000);
        CloseHandle(worker_thread); worker_thread = NULL;
    }
    if (music_buffer) { IDirectSoundBuffer_Release(music_buffer); music_buffer = NULL; }
    if (music_file != INVALID_HANDLE_VALUE) { CloseHandle(music_file); music_file = INVALID_HANDLE_VALUE; }
    if (lock_ready) { DeleteCriticalSection(&audio_lock); lock_ready = 0; }
}
