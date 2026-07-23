/* Embedded Win95 BGM transport. This is linked into Doraemon.exe, not shipped
   as a DLL. It creates its only DirectSound buffer through the game's original
   in-memory SFX loader and then streams mono PCM into that proven buffer. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>

#define TRACKS 10
#define HEADER_SIZE 192
#define SAMPLE_RATE 22050
#define BLOCK_FRAMES 4096
#define INVALID_FILE ((HANDLE)(intptr_t)-1)

typedef DWORD (__stdcall *GetModuleFileNameA_t)(HMODULE, char *, DWORD);
typedef HANDLE (__stdcall *CreateFileA_t)(const char *, DWORD, DWORD, void *, DWORD, DWORD, HANDLE);
typedef BOOL (__stdcall *ReadFile_t)(HANDLE, void *, DWORD, DWORD *, void *);
typedef DWORD (__stdcall *SetFilePointer_t)(HANDLE, LONG, LONG *, DWORD);
typedef BOOL (__stdcall *CloseHandle_t)(HANDLE);
typedef void (__stdcall *Sleep_t)(DWORD);
typedef UINT (__stdcall *timeBeginPeriod_t)(UINT);
typedef UINT (__stdcall *timeEndPeriod_t)(UINT);
typedef UINT (__stdcall *timeKillEvent_t)(UINT);
typedef UINT (__stdcall *timeSetEvent_t)(UINT, UINT, void *, DWORD, UINT);

#define IAT(type, address) (*(type *)(uintptr_t)(address))
#define GetModuleFileNameA_ IAT(GetModuleFileNameA_t, 0x004b90d0)
#define Sleep_              IAT(Sleep_t,              0x004b9154)
#define SetFilePointer_     IAT(SetFilePointer_t,     0x004b9190)
#define CreateFileA_        IAT(CreateFileA_t,        0x004b9198)
#define CloseHandle_        IAT(CloseHandle_t,        0x004b91a0)
#define ReadFile_           IAT(ReadFile_t,           0x004b91ac)
#define timeBeginPeriod_    IAT(timeBeginPeriod_t,    0x004b9264)
#define timeKillEvent_      IAT(timeKillEvent_t,      0x004b9290)
#define timeSetEvent_       IAT(timeSetEvent_t,       0x004b9298)
#define timeEndPeriod_      IAT(timeEndPeriod_t,      0x004b92a0)

typedef void *(__cdecl *GameAlloc)(DWORD);
typedef void (__cdecl *GameFree)(void *);
typedef int (__thiscall *ReserveSlot)(void *);
typedef int (__thiscall *ReleaseSlot)(void *, int);
typedef int (__thiscall *PlayMemory)(void *, int, void *, int);
#define game_alloc ((GameAlloc)0x0048cd91)
#define game_free ((GameFree)0x0048ce5c)
#define reserve_slot ((ReserveSlot)0x004896d9)
#define release_slot ((ReleaseSlot)0x004897b4)
#define play_memory ((PlayMemory)0x00489041)

typedef struct { DWORD id, offset, length, frames; } Track;
typedef struct {
    WORD format, channels;
    DWORD samples_per_second, bytes_per_second;
    WORD block_align, bits_per_sample, extra;
} WaveFormat;
typedef struct {
    DWORD size, flags, bytes, reserved;
    WaveFormat *format;
} BufferDescription;
typedef struct {
    HANDLE file;
    void *manager;
    void *buffer;
    BYTE *input;
    BYTE *scratch;
    DWORD input_at, input_len;
    UINT timer;
    volatile LONG lock;
    DWORD generation;
    DWORD level;
    DWORD sample_rate, half_frames, half_bytes, buffer_bytes;
    int active_track;
    int current_half;
    int playing;
    int full_track;
    Track tracks[TRACKS];
    DWORD track_offset, encoded_left, frames_left;
    DWORD block_left;
    int predictor, step_index, first_sample, high_nibble;
    BYTE packed;
} State;

static State state = { INVALID_FILE };

/* PE patch revisions are not safely replaceable in place. This marker lets
   the Rust patcher distinguish this streamer from the first embedded build. */
__declspec(dllexport) const char BgmRuntimeMarker[] = "BGMRT4";

static const DWORD track_frames[TRACKS] = {
    7525812,2865912,924924,2394924,174048,2130324,1541736,1376802,1903944,308994
};

static const int step_table[89] = {
    7,8,9,10,11,12,13,14,16,17,19,21,23,25,28,31,34,37,41,45,50,55,60,66,
    73,80,88,97,107,118,130,143,157,173,190,209,230,253,279,307,337,371,
    408,449,494,544,598,658,724,796,876,963,1060,1166,1282,1411,1552,1707,
    1878,2066,2272,2499,2749,3024,3327,3660,4026,4428,4871,5358,5894,6484,
    7132,7845,8630,9493,10442,11487,12635,13899,15289,16818,18500,20350,
    22385,24623,27086,29794,32767
};
static const signed char index_table[16] = {-1,-1,-1,-1,2,4,6,8,-1,-1,-1,-1,2,4,6,8};
static const LONG volume_table[54] = {
    -10000,-3449,-2847,-2495,-2245,-2051,-1893,-1759,-1643,-1541,-1450,
    -1367,-1291,-1221,-1156,-1095,-1038,-985,-934,-886,-840,-796,-754,
    -713,-674,-636,-600,-565,-531,-498,-466,-435,-405,-376,-347,-319,
    -292,-265,-239,-214,-189,-165,-142,-118,-95,-73,-51,-30,-8,0,0,0,0,0
};

static DWORD le32(const BYTE *p) {
    return (DWORD)p[0] | ((DWORD)p[1] << 8) | ((DWORD)p[2] << 16) | ((DWORD)p[3] << 24);
}
static WORD le16(const BYTE *p) { return (WORD)(p[0] | ((WORD)p[1] << 8)); }
static void put16(BYTE *p, WORD v) { p[0]=(BYTE)v; p[1]=(BYTE)(v>>8); }
static void put32(BYTE *p, DWORD v) { p[0]=(BYTE)v; p[1]=(BYTE)(v>>8); p[2]=(BYTE)(v>>16); p[3]=(BYTE)(v>>24); }
static void copy_bytes(BYTE *to, const BYTE *from, DWORD n) { while (n--) *to++=*from++; }
static void zero_bytes(BYTE *to, DWORD n) { while (n--) *to++=0; }
static int equal(const BYTE *a, const char *b, DWORD n) { while (n--) if (*a++ != (BYTE)*b++) return 0; return 1; }

static int try_lock(void) {
    LONG value = 1;
    __asm__ __volatile__("xchgl %0,%1" : "+r"(value), "+m"(state.lock) :: "memory");
    return value == 0;
}
static void acquire(void) { while (!try_lock()) Sleep_(0); }
static void unlock(void) { __asm__ __volatile__("" ::: "memory"); state.lock = 0; }

static int read_exact(void *target, DWORD length) {
    DWORD got = 0;
    return state.file != INVALID_FILE && ReadFile_(state.file, target, length, &got, 0) && got == length;
}
static int seek_file(DWORD offset) {
    state.input_at = state.input_len = 0;
    return SetFilePointer_(state.file, (LONG)offset, 0, FILE_BEGIN) != 0xffffffffUL;
}

/* The timer must never turn a 22 kHz stream into 22,000 Kernel32 calls per
   second.  Read a whole compressed chunk, then decode bytes from memory. */
static int read_cached(BYTE *target, DWORD length) {
    while (length) {
        DWORD available, take, got = 0;
        if (state.input_at == state.input_len) {
            take = state.encoded_left;
            if (take > state.half_bytes) take = state.half_bytes;
            if (!take || !ReadFile_(state.file, state.input, take, &got, 0) || got != take) return 0;
            state.input_at = 0;
            state.input_len = take;
        }
        available = state.input_len - state.input_at;
        take = length < available ? length : available;
        copy_bytes(target, state.input + state.input_at, take);
        state.input_at += take;
        target += take;
        length -= take;
    }
    return 1;
}

static int validate_tracks(void) {
    int i;
    for (i=0;i<TRACKS;++i) {
        DWORD encoded=state.tracks[i].length, frames=state.tracks[i].frames;
        if (!seek_file(state.tracks[i].offset)) return 0;
        while (frames) {
            BYTE h[6];
            DWORD expected=frames>BLOCK_FRAMES ? BLOCK_FRAMES : frames;
            DWORD packed=expected/2;
            if (encoded<6 || !read_exact(h,6) || le16(h)!=expected || h[4]>88 || h[5]) return 0;
            encoded-=6;
            if (encoded<packed || SetFilePointer_(state.file,(LONG)packed,0,FILE_CURRENT)==0xffffffffUL) return 0;
            encoded-=packed; frames-=expected;
        }
        if (encoded) return 0;
    }
    return 1;
}

static int reset_track(void) {
    Track *t;
    if (state.active_track < 0 || state.active_track >= TRACKS) return 0;
    t = &state.tracks[state.active_track];
    if (!seek_file(t->offset)) return 0;
    state.track_offset = t->offset;
    state.encoded_left = t->length;
    state.frames_left = t->frames;
    state.block_left = 0;
    state.first_sample = 0;
    state.high_nibble = 0;
    return 1;
}

static int load_block(void) {
    BYTE h[6];
    DWORD count;
    if (state.frames_left == 0 && !reset_track()) return 0;
    if (state.encoded_left < 6 || !read_cached(h, 6)) return 0;
    state.encoded_left -= 6;
    count = le16(h);
    if (!count || count > BLOCK_FRAMES || count > state.frames_left || h[4] > 88 || h[5]) return 0;
    state.predictor = (int16_t)le16(h + 2);
    state.step_index = h[4];
    state.block_left = count;
    state.first_sample = 1;
    state.high_nibble = 0;
    return 1;
}

static int next_sample(int16_t *sample) {
    int nibble, step, difference;
    if (!state.block_left && !load_block()) return 0;
    if (state.first_sample) {
        state.first_sample = 0;
        --state.block_left;
        --state.frames_left;
        *sample = (int16_t)state.predictor;
        return 1;
    }
    if (!state.high_nibble) {
        if (!state.encoded_left || !read_cached(&state.packed, 1)) return 0;
        --state.encoded_left;
        nibble = state.packed & 15;
        state.high_nibble = 1;
    } else {
        nibble = state.packed >> 4;
        state.high_nibble = 0;
    }
    step = step_table[state.step_index];
    difference = step >> 3;
    if (nibble & 1) difference += step >> 2;
    if (nibble & 2) difference += step >> 1;
    if (nibble & 4) difference += step;
    if (nibble & 8) state.predictor -= difference; else state.predictor += difference;
    if (state.predictor < -32768) state.predictor = -32768;
    if (state.predictor > 32767) state.predictor = 32767;
    state.step_index += index_table[nibble];
    if (state.step_index < 0) state.step_index = 0;
    if (state.step_index > 88) state.step_index = 88;
    --state.block_left;
    --state.frames_left;
    *sample = (int16_t)state.predictor;
    return 1;
}

static int decode(BYTE *target, DWORD frames) {
    DWORD i;
    for (i = 0; i < frames; ++i) {
        int16_t sample;
        if (!next_sample(&sample)) { zero_bytes(target + i * 2, (frames - i) * 2); return 0; }
        target[i * 2] = (BYTE)sample;
        target[i * 2 + 1] = (BYTE)((uint16_t)sample >> 8);
    }
    return 1;
}

static HRESULT com0(void *object, int index) {
    typedef HRESULT (__stdcall *Fn)(void *);
    return ((Fn)(*(void ***)object)[index])(object);
}
static HRESULT com1(void *object, int index, DWORD value) {
    typedef HRESULT (__stdcall *Fn)(void *, DWORD);
    return ((Fn)(*(void ***)object)[index])(object, value);
}
static HRESULT play_buffer(void *object) {
    typedef HRESULT (__stdcall *Fn)(void *, DWORD, DWORD, DWORD);
    return ((Fn)(*(void ***)object)[12])(object, 0, 0, 1);
}
static HRESULT get_position(void *object, DWORD *play) {
    typedef HRESULT (__stdcall *Fn)(void *, DWORD *, DWORD *);
    return ((Fn)(*(void ***)object)[4])(object, play, 0);
}
static HRESULT lock_buffer(void *object, DWORD offset, DWORD length, void **a, DWORD *an, void **b, DWORD *bn) {
    typedef HRESULT (__stdcall *Fn)(void *,DWORD,DWORD,void**,DWORD*,void**,DWORD*,DWORD);
    return ((Fn)(*(void ***)object)[11])(object,offset,length,a,an,b,bn,0);
}
static HRESULT unlock_buffer(void *object, void *a, DWORD an, void *b, DWORD bn) {
    typedef HRESULT (__stdcall *Fn)(void *,void*,DWORD,void*,DWORD);
    return ((Fn)(*(void ***)object)[19])(object,a,an,b,bn);
}
static void set_volume(void);

static int create_full_buffer(DWORD frames) {
    BufferDescription description;
    WaveFormat format;
    void *device, *a=0, *b=0;
    DWORD bytes=frames*2, an=0, bn=0;
    typedef HRESULT (__stdcall *CreateSoundBufferFn)(void *,BufferDescription *,void **,void *);
    device=*(void **)((BYTE*)state.manager+0x10c);
    if (!device) return 0;
    format.format=1; format.channels=1; format.samples_per_second=state.sample_rate;
    format.bytes_per_second=state.sample_rate*2; format.block_align=2;
    format.bits_per_sample=16; format.extra=0;
    description.size=sizeof(description);
    description.flags=*(DWORD *)((BYTE*)state.manager+0xe8);
    description.bytes=bytes; description.reserved=0; description.format=&format;
    if (((CreateSoundBufferFn)(*(void ***)device)[3])(device,&description,&state.buffer,0) != 0 ||
        !state.buffer) { state.buffer=0; return 0; }
    if (lock_buffer(state.buffer,0,bytes,&a,&an,&b,&bn) != 0) {
        com0(state.buffer,2); state.buffer=0; return 0;
    }
    if (!decode((BYTE*)a,an/2) || (b && bn && !decode((BYTE*)b,bn/2))) {
        unlock_buffer(state.buffer,a,an,b,bn);
        com0(state.buffer,2); state.buffer=0; return 0;
    }
    if (unlock_buffer(state.buffer,a,an,b,bn) != 0) {
        com0(state.buffer,2); state.buffer=0; return 0;
    }
    if (com1(state.buffer,13,0) != 0 || play_buffer(state.buffer) != 0) {
        com0(state.buffer,2); state.buffer=0; return 0;
    }
    set_volume();
    return 1;
}
static void set_volume(void) {
    DWORD index = (state.level * 53UL) / 65535UL;
    if (state.buffer) com1(state.buffer, 15, (DWORD)volume_table[index]);
}

static int refill(DWORD offset, DWORD frames) {
    void *a=0,*b=0; DWORD an=0,bn=0;
    if (!decode(state.scratch, frames)) return 0;
    if (lock_buffer(state.buffer, offset, frames*2, &a,&an,&b,&bn) != 0) {
        com0(state.buffer, 20);
        if (lock_buffer(state.buffer, offset, frames*2, &a,&an,&b,&bn) != 0) return 0;
    }
    copy_bytes((BYTE*)a, state.scratch, an);
    if (b && bn) copy_bytes((BYTE*)b, state.scratch + an, bn);
    return unlock_buffer(state.buffer,a,an,b,bn) == 0;
}

static void wave_header(BYTE *p, DWORD bytes) {
    copy_bytes(p,(const BYTE*)"RIFF",4); put32(p+4,bytes+36);
    copy_bytes(p+8,(const BYTE*)"WAVEfmt ",8); put32(p+16,16); put16(p+20,1); put16(p+22,1);
    put32(p+24,state.sample_rate); put32(p+28,state.sample_rate*2); put16(p+32,2); put16(p+34,16);
    copy_bytes(p+36,(const BYTE*)"data",4); put32(p+40,bytes);
}

static int create_buffer(DWORD frames) {
    BYTE *seed;
    DWORD bytes=frames*2;
    int slot;
    void **owned;
    seed = (BYTE*)game_alloc(bytes + 44);
    if (!seed) return 0;
    wave_header(seed,bytes);
    if (!decode(seed + 44, frames)) { game_free(seed); return 0; }
    slot = reserve_slot(state.manager);
    if (slot < 0 || slot >= 8) { game_free(seed); return 0; }
    if (!play_memory(state.manager, slot, seed, 1)) {
        release_slot(state.manager, slot); game_free(seed); return 0;
    }
    owned = (void **)((BYTE*)state.manager + 0x1c + slot * 0x18);
    state.buffer = *owned;
    *owned = 0;
    release_slot(state.manager, slot);
    game_free(seed);
    /* The SFX loader starts its seed as a one-shot effect.  Once ownership is
       detached, restart that same buffer in looping mode for the streamer. */
    if (!state.buffer || com0(state.buffer,18) != 0 || com1(state.buffer,13,0) != 0 ||
        play_buffer(state.buffer) != 0) {
        if (state.buffer) { com0(state.buffer,2); state.buffer=0; }
        return 0;
    }
    set_volume();
    return 1;
}

static void __stdcall BgmTimerCallback(UINT id, UINT message, DWORD user, DWORD first, DWORD second) {
    DWORD cursor, half;
    (void)id; (void)message; (void)first; (void)second;
    if (!try_lock()) return;
    if (user==state.generation && state.playing &&
        state.buffer && get_position(state.buffer,&cursor) == 0) {
        half = cursor >= state.half_bytes;
        if ((int)half != state.current_half) {
            if (refill(half ? 0 : state.half_bytes, state.half_frames)) state.current_half = (int)half;
            else state.playing = 0;
        }
    }
    unlock();
}

static void stop_timer(void) {
    if (state.timer) { timeKillEvent_(state.timer); state.timer=0; timeEndPeriod_(1); }
}

static int start_timer(void) {
    if (state.timer) return 1;
    timeBeginPeriod_(1);
    state.timer=timeSetEvent_(250,10,(void*)BgmTimerCallback,state.generation,1);
    if (!state.timer) { timeEndPeriod_(1); return 0; }
    return 1;
}

static void close_locked(void) {
    ++state.generation;
    state.playing = 0;
    stop_timer();
    if (state.buffer) { com0(state.buffer,18); com0(state.buffer,2); state.buffer=0; }
    if (state.input) { game_free(state.input); state.input=0; }
    if (state.scratch) { game_free(state.scratch); state.scratch=0; }
    state.input_at = state.input_len = 0;
    if (state.file != INVALID_FILE) { CloseHandle_(state.file); state.file=INVALID_FILE; }
    state.active_track=-1;
}

__declspec(dllexport) DWORD __stdcall BgmInit(void *manager) {
    BYTE h[HEADER_SIZE]; char path[MAX_PATH]; DWORD n,size,previous=HEADER_SIZE; int i;
    acquire(); close_locked(); state.manager=manager; state.level=65535;
    n=GetModuleFileNameA_(0,path,MAX_PATH);
    if (!n || n>=MAX_PATH) { unlock(); return 0; }
    while (n && path[n-1]!='\\' && path[n-1]!='/') --n;
    copy_bytes((BYTE*)path+n,(const BYTE*)"BGM.dat",8); path[n+7]=0;
    state.file=CreateFileA_(path,GENERIC_READ,FILE_SHARE_READ,0,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL,0);
    if (state.file==INVALID_FILE || !read_exact(h,HEADER_SIZE)) { close_locked(); unlock(); return 0; }
    size=SetFilePointer_(state.file,0,0,FILE_END);
    state.sample_rate=le32(h+16);
    if (!equal(h,"DBGM1\0\0\0",8) || le32(h+8)!=1 || le32(h+12)!=TRACKS ||
        (state.sample_rate!=22050 && state.sample_rate!=14700 && state.sample_rate!=11025) ||
        le16(h+20)!=1 || le16(h+22)!=16 || le32(h+24)!=BLOCK_FRAMES) {
        close_locked(); unlock(); return 0;
    }
    state.half_frames=state.sample_rate*2;
    state.half_bytes=state.half_frames*2;
    state.buffer_bytes=state.half_bytes*2;
    for (i=0;i<TRACKS;++i) {
        BYTE *e=h+32+i*16; Track *t=&state.tracks[i];
        t->id=le32(e); t->offset=le32(e+4); t->length=le32(e+8); t->frames=le32(e+12);
        DWORD expected=state.sample_rate==22050 ? track_frames[i] :
            (state.sample_rate==14700 ? track_frames[i]*2/3 : track_frames[i]/2);
        if (t->id!=(DWORD)(i+2) || t->offset!=previous || !t->length || t->frames!=expected ||
            t->offset+t->length<t->offset || t->offset+t->length>size) { close_locked(); unlock(); return 0; }
        previous=t->offset+t->length;
    }
    if (previous!=size || !validate_tracks() ||
        !(state.input=(BYTE*)game_alloc(state.half_bytes)) ||
        !(state.scratch=(BYTE*)game_alloc(state.half_bytes))) { close_locked(); unlock(); return 0; }
    start_timer();
    unlock(); return 1;
}

__declspec(dllexport) DWORD __stdcall BgmPlay(DWORD id) {
    int ok=0;
    acquire();
    if (id>=2 && id<=11 && state.file!=INVALID_FILE && state.scratch) {
        if (state.buffer) com0(state.buffer,18);
        state.active_track=(int)id-2;
        if (reset_track()) {
            if (!state.buffer) ok=create_buffer(state.buffer_bytes/2);
            else ok=refill(0, state.half_frames) && refill(state.half_bytes, state.half_frames) &&
                com0(state.buffer,18)==0 && com1(state.buffer,13,0)==0 && play_buffer(state.buffer)==0;
            if (ok && !state.timer) {
                ok=start_timer();
            }
        }
    }
    state.playing=ok; state.current_half=0; unlock(); return ok;
}
__declspec(dllexport) DWORD __stdcall BgmStop(void) { acquire(); state.playing=0; if(state.buffer)com0(state.buffer,18); unlock(); return 1; }
__declspec(dllexport) DWORD __stdcall BgmDuration(DWORD id) {
    DWORD result=0; acquire(); if(id>=2&&id<=11) result=(state.tracks[id-2].frames+state.sample_rate-1)/state.sample_rate; unlock(); return result;
}
__declspec(dllexport) DWORD __stdcall BgmCount(void) { return 11; }
__declspec(dllexport) DWORD __stdcall BgmVolume(DWORD level) { acquire(); state.level=level; set_volume(); unlock(); return 1; }
__declspec(dllexport) DWORD __stdcall BgmGetVolume(void) { return state.level; }
__declspec(dllexport) DWORD __stdcall BgmClose(void) { acquire(); close_locked(); unlock(); return 1; }
__declspec(dllexport) DWORD __stdcall BgmDispatch(DWORD command, DWORD first, DWORD second) {
    (void)second;
    switch (command) {
        case 0: return BgmInit((void *)(uintptr_t)first);
        case 1: return BgmPlay(first);
        case 2: return BgmStop();
        case 3: return BgmDuration(first);
        case 4: return BgmCount();
        case 5: return BgmVolume(first);
        case 6: return BgmClose();
        case 7: return BgmGetVolume();
        default: return 0;
    }
}
