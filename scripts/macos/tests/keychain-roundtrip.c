#include <Security/Security.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#ifndef VERSION
#error VERSION must distinguish the two builds
#endif

int main(int argc, char **argv) {
    if (argc != 2) return 10;
    SecKeychainRef keychain = NULL;
    if (SecKeychainSetUserInteractionAllowed(false) != errSecSuccess) return 11;
    if (SecKeychainOpen(argv[1], &keychain) != errSecSuccess) return 12;
    const char service[] = "com.lettuceai.signing-smoke";
    const char account[] = "cross-build-grant";
    const char value[] = "signing-smoke-canary";
    OSStatus status;
#if VERSION == 1
    status = SecKeychainAddGenericPassword(keychain, sizeof(service) - 1, service,
        sizeof(account) - 1, account, sizeof(value) - 1, value, NULL);
#else
    UInt32 length = 0;
    void *bytes = NULL;
    SecKeychainItemRef item = NULL;
    status = SecKeychainFindGenericPassword(keychain, sizeof(service) - 1, service,
        sizeof(account) - 1, account, &length, &bytes, &item);
    if (status == errSecSuccess) {
        int matches = length == sizeof(value) - 1 && memcmp(bytes, value, length) == 0;
        SecKeychainItemFreeContent(NULL, bytes);
        status = SecKeychainItemDelete(item);
        CFRelease(item);
        if (!matches) status = errSecDecode;
    }
#endif
    CFRelease(keychain);
    if (status != errSecSuccess) fprintf(stderr, "Keychain smoke test failed: %d\n", (int)status);
    return status == errSecSuccess ? 0 : 13;
}
