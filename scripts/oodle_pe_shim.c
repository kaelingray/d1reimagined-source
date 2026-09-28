/* Minimal, contained loader for a user-supplied Oodle Windows DLL (oo2core_3_win64.dll) on
 * Linux x86-64, so scripts/d1_pkg.py can decompress Destiny package blocks outside Windows.
 *
 * It maps the PE image, applies its base relocations and resolves every import to a stub
 * written here. Only the C-runtime services the decoder needs are implemented (heap, TLS,
 * critical sections as no-ops, time, error codes); every other import - file, module, process
 * and console functions - aborts with its name, so the DLL cannot reach anything outside this
 * process except through these stubs. DllMain (C runtime start-up) is never run. Calls use the
 * Windows x64 convention (gcc ms_abi).
 *
 * Build: gcc -O2 -shared -fPIC -o local/liboodle_shim.so scripts/oodle_pe_shim.c
 * API:   int d1_oodle_load(const char *dll_path)            0 = ok
 *        long long d1_oodle_decompress(src, srclen, dst, dstlen)   OodleLZ_Decompress result
 * Never commit the DLL itself. */
#define _GNU_SOURCE
#include <malloc.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <asm/prctl.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#define WINAPI __attribute__((ms_abi))
typedef int64_t (WINAPI *decompress_fn)(const void *, int64_t, void *, int64_t, int, int, int, void *,
                                        int64_t, void *, void *, void *, int64_t, int);
static decompress_fn g_decompress;
static uint32_t g_last_error;

/* --- stubs ------------------------------------------------------------------------------- */
static WINAPI void *s_GetProcessHeap(void) { return (void *)0x1000; }
static WINAPI void *s_HeapAlloc(void *h, uint32_t flags, size_t n) {
    (void)h; void *p = malloc(n ? n : 1); if (p && (flags & 8)) memset(p, 0, n); return p; }
static WINAPI int s_HeapFree(void *h, uint32_t f, void *p) { (void)h; (void)f; free(p); return 1; }
static WINAPI void *s_HeapReAlloc(void *h, uint32_t flags, void *p, size_t n) {
    (void)h; size_t old = malloc_usable_size(p); void *q = realloc(p, n ? n : 1);
    if (q && (flags & 8) && n > old)
        memset((char *)q + old, 0, n - old);
    return q;
}
static WINAPI size_t s_HeapSize(void *h, uint32_t f, const void *p) { (void)h; (void)f; return malloc_usable_size((void *)p); }
static void *g_tls[64]; static int g_tls_used[64];
static WINAPI uint32_t s_TlsAlloc(void) { for (int i = 0; i < 64; i++) if (!g_tls_used[i]) { g_tls_used[i] = 1; return i; } return 0xffffffff; }
static WINAPI void *s_TlsGetValue(uint32_t i) { return i < 64 ? g_tls[i] : NULL; }
static WINAPI int s_TlsSetValue(uint32_t i, void *v) { if (i >= 64) return 0; g_tls[i] = v; return 1; }
static WINAPI int s_TlsFree(uint32_t i) { if (i < 64) g_tls_used[i] = 0; return 1; }
static WINAPI void s_nop1(void *p) { (void)p; }                         /* Enter/Leave/DeleteCriticalSection */
static WINAPI int s_InitCS(void *p, uint32_t spin) { (void)p; (void)spin; return 1; }
static WINAPI uint32_t s_GetLastError(void) { return g_last_error; }
static WINAPI void s_SetLastError(uint32_t e) { g_last_error = e; }
static WINAPI void *s_identity(void *p) { return p; }                  /* Encode/DecodePointer */
static WINAPI uint32_t s_GetCurrentThreadId(void) { return 1; }
static WINAPI uint32_t s_GetCurrentProcessId(void) { return (uint32_t)getpid(); }
static WINAPI void *s_GetCurrentProcess(void) { return (void *)-1; }
static WINAPI int s_IsDebuggerPresent(void) { return 0; }
static WINAPI int s_IsProcessorFeaturePresent(uint32_t f) { (void)f; return 0; }
static WINAPI int s_QueryPerformanceCounter(int64_t *c) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); *c = t.tv_sec * 1000000000LL + t.tv_nsec; return 1; }
static WINAPI void s_GetSystemTimeAsFileTime(uint64_t *ft) { *ft = (uint64_t)time(NULL) * 10000000ULL + 116444736000000000ULL; }
static WINAPI void s_Sleep(uint32_t ms) { usleep(ms * 1000); }
static WINAPI void s_OutputDebugStringA(const char *s) { fprintf(stderr, "oodle: %s", s); }
static WINAPI void s_OutputDebugStringW(const void *s) { (void)s; }

static const char *g_unknown[128];
static void unknown_import(int i) {
    fprintf(stderr, "oodle shim: the DLL called unsupported import %s; refusing\n", g_unknown[i]);
    abort();
}
#define U(n) static WINAPI uint64_t u##n(void) { unknown_import(n); return 0; }
#define U8(a) U(a##0) U(a##1) U(a##2) U(a##3) U(a##4) U(a##5) U(a##6) U(a##7)
U8(1) U8(2) U8(3) U8(4) U8(5) U8(6) U8(7) U8(8)
#define P8(a) (void *)u##a##0, (void *)u##a##1, (void *)u##a##2, (void *)u##a##3, \
              (void *)u##a##4, (void *)u##a##5, (void *)u##a##6, (void *)u##a##7
static void *g_unknown_stubs[] = {P8(1), P8(2), P8(3), P8(4), P8(5), P8(6), P8(7), P8(8)};
/* the numbering above makes stub k print g_unknown[k]; map slots in load order */
static int unknown_index(int slot) {
    int tens = slot / 8 + 1, ones = slot % 8;
    return tens * 10 + ones;
}

static const struct { const char *name; void *fn; } k_stubs[] = {
    {"GetProcessHeap", s_GetProcessHeap}, {"HeapAlloc", s_HeapAlloc}, {"HeapFree", s_HeapFree},
    {"HeapReAlloc", s_HeapReAlloc}, {"HeapSize", s_HeapSize}, {"TlsAlloc", s_TlsAlloc},
    {"TlsGetValue", s_TlsGetValue}, {"TlsSetValue", s_TlsSetValue}, {"TlsFree", s_TlsFree},
    {"EnterCriticalSection", s_nop1}, {"LeaveCriticalSection", s_nop1}, {"DeleteCriticalSection", s_nop1},
    {"InitializeCriticalSectionAndSpinCount", s_InitCS}, {"GetLastError", s_GetLastError},
    {"SetLastError", s_SetLastError}, {"EncodePointer", s_identity}, {"DecodePointer", s_identity},
    {"GetCurrentThreadId", s_GetCurrentThreadId}, {"GetCurrentProcessId", s_GetCurrentProcessId},
    {"GetCurrentProcess", s_GetCurrentProcess}, {"IsDebuggerPresent", s_IsDebuggerPresent},
    {"IsProcessorFeaturePresent", s_IsProcessorFeaturePresent},
    {"QueryPerformanceCounter", s_QueryPerformanceCounter},
    {"GetSystemTimeAsFileTime", s_GetSystemTimeAsFileTime}, {"Sleep", s_Sleep},
    {"OutputDebugStringA", s_OutputDebugStringA}, {"OutputDebugStringW", s_OutputDebugStringW},
};

/* --- loader -------------------------------------------------------------------------------- */
#define RD16(p) (*(const uint16_t *)(p))
#define RD32(p) (*(const uint32_t *)(p))
#define RD64(p) (*(const uint64_t *)(p))

/* MSVC x64 code reads the thread environment block through gs (e.g. __chkstk takes the stack
 * limit from gs:[0x10]). glibc on x86-64 uses fs only, so point gs at a small fake block:
 * StackBase = all ones, StackLimit = 0 (no probing needed), Self at +0x30. */
static uint64_t g_teb[0x200];
static int set_fake_teb(void) {
    g_teb[1] = ~0ULL;                  /* +0x08 StackBase */
    g_teb[2] = 0;                      /* +0x10 StackLimit */
    g_teb[6] = (uint64_t)(uintptr_t)g_teb; /* +0x30 Self */
    return (int)syscall(SYS_arch_prctl, ARCH_SET_GS, (unsigned long)g_teb);
}

int d1_oodle_load(const char *path) {
    if (set_fake_teb() != 0) return -8;
    FILE *f = fopen(path, "rb");
    if (!f) return -1;
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    unsigned char *file = malloc(size);
    if (!file || fread(file, 1, size, f) != (size_t)size) { fclose(f); return -2; }
    fclose(f);
    if (RD16(file) != 0x5a4d) return -3;
    const unsigned char *nt = file + RD32(file + 0x3c);
    if (RD32(nt) != 0x4550 || RD16(nt + 4) != 0x8664) return -4;
    uint16_t nsect = RD16(nt + 6), optsize = RD16(nt + 20);
    const unsigned char *opt = nt + 24;
    uint64_t image_base = RD64(opt + 24);
    uint32_t image_size = RD32(opt + 56), headers_size = RD32(opt + 60);
    const unsigned char *dirs = opt + 112;
    unsigned char *base = mmap(NULL, image_size, PROT_READ | PROT_WRITE | PROT_EXEC,
                               MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (base == MAP_FAILED) return -5;
    memcpy(base, file, headers_size);
    const unsigned char *sect = opt + optsize;
    for (int i = 0; i < nsect; i++, sect += 40) {
        uint32_t va = RD32(sect + 12), raw = RD32(sect + 16), off = RD32(sect + 20);
        if (raw) memcpy(base + va, file + off, raw);
    }
    /* base relocations (DIR64 only on x64) */
    uint32_t rel_va = RD32(dirs + 5 * 8), rel_size = RD32(dirs + 5 * 8 + 4);
    int64_t delta = (int64_t)(uintptr_t)base - (int64_t)image_base;
    for (uint32_t pos = 0; pos < rel_size;) {
        uint32_t page = RD32(base + rel_va + pos), block = RD32(base + rel_va + pos + 4);
        if (!block) break;
        for (uint32_t k = 8; k < block; k += 2) {
            uint16_t e = RD16(base + rel_va + pos + k);
            if ((e >> 12) == 10) *(uint64_t *)(base + page + (e & 0xfff)) += delta;
        }
        pos += block;
    }
    /* imports */
    uint32_t imp_va = RD32(dirs + 1 * 8);
    int unknown = 0;
    for (const unsigned char *d = base + imp_va; RD32(d + 12); d += 20) {
        uint32_t lookup = RD32(d) ? RD32(d) : RD32(d + 16), iat = RD32(d + 16);
        for (int j = 0;; j++) {
            uint64_t ent = RD64(base + lookup + 8 * j);
            if (!ent) break;
            void *fn = NULL;
            const char *name = (ent >> 63) ? "(ordinal)" : (const char *)(base + (uint32_t)ent + 2);
            for (size_t s = 0; s < sizeof k_stubs / sizeof *k_stubs; s++)
                if (!strcmp(name, k_stubs[s].name)) fn = k_stubs[s].fn;
            if (!fn) {
                if (unknown >= 64) return -6;
                g_unknown[unknown_index(unknown)] = name;
                fn = g_unknown_stubs[unknown++];
            }
            *(void **)(base + iat + 8 * j) = fn;
        }
    }
    /* export OodleLZ_Decompress */
    uint32_t exp_va = RD32(dirs);
    const unsigned char *ed = base + exp_va;
    uint32_t nnames = RD32(ed + 24), funcs = RD32(ed + 28), names = RD32(ed + 32), ords = RD32(ed + 36);
    for (uint32_t i = 0; i < nnames; i++) {
        if (!strcmp((const char *)base + RD32(base + names + 4 * i), "OodleLZ_Decompress")) {
            uint16_t o = RD16(base + ords + 2 * i);
            g_decompress = (decompress_fn)(base + RD32(base + funcs + 4 * o));
        }
    }
    free(file);
    return g_decompress ? 0 : -7;
}

long long d1_oodle_decompress(const void *src, long long srclen, void *dst, long long dstlen) {
    if (!g_decompress) return -1;
    /* fuzz-safe, no CRC check, no verbosity, no callbacks, all thread phases (as tiger-pkg) */
    return g_decompress(src, srclen, dst, dstlen, 1, 0, 0, NULL, 0, NULL, NULL, NULL, 0, 3);
}
