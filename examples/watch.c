/* Print keyboard events using the C API.
 *   Windows (MSVC): cl examples\watch.c /Iinclude target\release\manykey.dll.lib
 *   Linux:          cc examples/watch.c -Iinclude -Ltarget/release -lmanykey
 */
#include <stdio.h>
#include "manykey.h"

int main(void) {
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
