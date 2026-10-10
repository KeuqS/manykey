/* Print keyboard events using the C API.
 *   Windows (MSVC): cl examples\watch.c /Iinclude target\release\manykey.dll.lib
 *   Linux:          cc examples/watch.c -Iinclude -Ltarget/release -lmanykey
 */
#include <stdint.h>
#include <stdio.h>
#include "manykey.h"

int main(void) {
    /* Before 1.0 the header and the library must agree on major.minor. */
    uint32_t lib = manykey_version();
    if ((lib >> 8) != (MANYKEY_VERSION >> 8)) {
        fprintf(stderr, "manykey: built against %u.%u.%u but loaded %u.%u.%u\n",
                (unsigned)MANYKEY_VERSION_MAJOR, (unsigned)MANYKEY_VERSION_MINOR,
                (unsigned)MANYKEY_VERSION_PATCH, (unsigned)(lib >> 16),
                (unsigned)((lib >> 8) & 0xff), (unsigned)(lib & 0xff));
        return 1;
    }

    manykey_context *ctx;
    if (manykey_open(&ctx) != 0) {
        fprintf(stderr, "manykey: %s\n", manykey_last_error());
        return 1;
    }
    manykey_event ev = { .size = sizeof ev };
    while (manykey_wait(ctx, &ev, -1) == 1) {
        if (ev.type == MANYKEY_EVENT_KEYBOARD_ADDED) {
            manykey_keyboard_info info = { .size = sizeof info };
            if (manykey_get_keyboard_info(ctx, ev.keyboard_id, &info) == 0)
                printf("+ keyboard %llu %s\n", (unsigned long long)info.id, info.name);
        } else if (ev.type == MANYKEY_EVENT_KEYBOARD_REMOVED) {
            printf("- keyboard %llu\n", (unsigned long long)ev.keyboard_id);
        } else {
            char name[32];
            manykey_key_name(ev.key, name, sizeof name);
            printf("keyboard %llu %s %s\n", (unsigned long long)ev.keyboard_id, name,
                   ev.state == MANYKEY_KEY_RELEASED ? "up" : "down");
        }
    }
    manykey_close(ctx);
    return 0;
}
