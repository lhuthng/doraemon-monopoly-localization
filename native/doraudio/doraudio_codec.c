/*
 * Music.dat decoder.
 *
 * This module turns compact stereo IMA-ADPCM blocks into PCM samples. That
 * conversion is needed because DirectSound accepts PCM buffers, while
 * Music.dat stores smaller decoder-friendly blocks. Each block begins with
 * predictor/index state and each following byte supplies one nibble per
 * channel. Invalid or truncated input becomes silence so the game continues
 * running without music.
 *
 * This file is included by doraudio.c after the shared state and constants.
 * Keeping it as a separate source makes the binary codec auditable without
 * changing the single-object MinGW build used by the release builder.
 */

static int read_exact(void *target, DWORD length) {
    DWORD read = 0;
    return ReadFile(music_file, target, length, &read, NULL) && read == length;
}

/* Decodes one ADPCM nibble and advances the channel predictor state. */
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

/* Reads one block header and initializes the two channel decoders. */
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

/* Fills an interleaved PCM destination, padding malformed input with silence. */
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
