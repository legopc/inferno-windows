#pragma once
#include <wdm.h>

// INFERNO: shared memory bridge
#define INFERNO_SHM_NAME L"\\BaseNamedObjects\\InfernoAudioShm"
#define INFERNO_SHM_BUFFER_FRAMES 4096
#define INFERNO_SHM_CHANNELS 8
#define INFERNO_SHM_HEADER_SIZE 16  // 4 x ULONG

typedef struct _INFERNO_SHM_HEADER {
    volatile LONG write_index;
    ULONG channel_count;
    ULONG sample_rate;
    ULONG buffer_frames;
} INFERNO_SHM_HEADER, *PINFERNO_SHM_HEADER;

// Global shared memory state (defined in shm.cpp)
extern PVOID g_ShmBase;
extern HANDLE g_ShmHandle;

NTSTATUS InfernoShmInit(ULONG channelCount, ULONG sampleRate);
VOID InfernoShmWrite(const INT16* samples, ULONG frameCount, ULONG channelCount);
VOID InfernoShmCleanup(void);
