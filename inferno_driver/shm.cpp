// INFERNO: shared memory bridge
#include "shm.h"

PVOID g_ShmBase = nullptr;
HANDLE g_ShmHandle = nullptr;

NTSTATUS InfernoShmInit(ULONG channelCount, ULONG sampleRate) {
    UNICODE_STRING name;
    RtlInitUnicodeString(&name, INFERNO_SHM_NAME);
    
    OBJECT_ATTRIBUTES objAttr;
    InitializeObjectAttributes(&objAttr, &name, OBJ_KERNEL_HANDLE | OBJ_CASE_INSENSITIVE, NULL, NULL);
    
    SIZE_T size = INFERNO_SHM_HEADER_SIZE + 
                  INFERNO_SHM_BUFFER_FRAMES * channelCount * sizeof(INT16);
    
    LARGE_INTEGER maxSize;
    maxSize.QuadPart = (LONGLONG)size;
    
    NTSTATUS status = ZwCreateSection(&g_ShmHandle, SECTION_ALL_ACCESS, &objAttr,
                                       &maxSize, PAGE_READWRITE, SEC_COMMIT, NULL);
    if (!NT_SUCCESS(status)) return status;
    
    SIZE_T viewSize = size;
    status = ZwMapViewOfSection(g_ShmHandle, ZwCurrentProcess(), &g_ShmBase,
                                 0, size, NULL, &viewSize, ViewUnmap, 0, PAGE_READWRITE);
    if (!NT_SUCCESS(status)) {
        ZwClose(g_ShmHandle);
        g_ShmHandle = nullptr;
        return status;
    }
    
    // Initialize header
    PINFERNO_SHM_HEADER hdr = (PINFERNO_SHM_HEADER)g_ShmBase;
    hdr->write_index = 0;
    hdr->channel_count = channelCount;
    hdr->sample_rate = sampleRate;
    hdr->buffer_frames = INFERNO_SHM_BUFFER_FRAMES;
    
    return STATUS_SUCCESS;
}

VOID InfernoShmWrite(const INT16* samples, ULONG frameCount, ULONG channelCount) {
    if (!g_ShmBase) return;
    
    PINFERNO_SHM_HEADER hdr = (PINFERNO_SHM_HEADER)g_ShmBase;
    INT16* ringBuf = (INT16*)((PUCHAR)g_ShmBase + INFERNO_SHM_HEADER_SIZE);
    
    LONG writeIdx = InterlockedAdd(&hdr->write_index, 0); // read current
    for (ULONG f = 0; f < frameCount; f++) {
        ULONG frameSlot = (writeIdx + f) % INFERNO_SHM_BUFFER_FRAMES;
        for (ULONG c = 0; c < channelCount; c++) {
            ringBuf[frameSlot * channelCount + c] = samples[f * channelCount + c];
        }
    }
    InterlockedAdd(&hdr->write_index, (LONG)frameCount);
}

VOID InfernoShmCleanup(void) {
    if (g_ShmBase) { ZwUnmapViewOfSection(ZwCurrentProcess(), g_ShmBase); g_ShmBase = nullptr; }
    if (g_ShmHandle) { ZwClose(g_ShmHandle); g_ShmHandle = nullptr; }
}
