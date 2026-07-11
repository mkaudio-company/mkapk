// Minimal, non-interactive driver for the real AAX Plug-In Validator's C API
// (AAXValidator.framework), used instead of the interactive DigiShell `dsh`
// REPL (which has no scriptable one-shot mode and hung this session once
// already when probed blindly). Built and run directly by
// `xtask/src/aax.rs` -- not part of the plugin's own CMake build.
//
// `AAXVal_DoTestsOnPlugIns`'s own `inTimeoutSeconds` argument is the safety
// net against a hung test process (the framework spawns and monitors a
// subprocess per test internally); this harness adds no timeout of its own
// on top of that.
#include <AAXValidator.h>

#include <cstdio>
#include <cstring>
#include <vector>

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s <path-to-.aaxplugin>\n", argv[0]);
        return 2;
    }

    AAXVal_Initialize();

    const char* pluginPaths[] = {argv[1]};
    const char* testIds[] = {
        "test.data_model",
        "test.load_unload",
        "test.parameters",
        "test.parameter_traversal.linear",
        "test.page_table.load",
        "test.describe_validation",
    };
    const int32_t numTests = static_cast<int32_t>(sizeof(testIds) / sizeof(testIds[0]));

    printf("Calling AAXVal_DoTestsOnPlugIns on %s with %d tests (timeout 60s)...\n", argv[1],
           numTests);
    AAXVal_Result result = AAXVal_DoTestsOnPlugIns(testIds, numTests, pluginPaths, 1, 60);
    printf("AAXVal_DoTestsOnPlugIns returned %d\n", result);

    int32_t maxSize = 0;
    AAXVal_GetMaxTestResultSize(kAAXVal_Format_JSON, &maxSize);

    std::vector<char> buf(static_cast<size_t>(maxSize) + 1, 0);
    int32_t numPassed = 0;
    for (int32_t i = 0; i < numTests; ++i) {
        std::fill(buf.begin(), buf.end(), 0);
        AAXVal_Result r =
            AAXVal_GetTestResult(kAAXVal_Format_JSON, i, buf.data(), static_cast<int32_t>(buf.size()));
        const bool passed = std::strstr(buf.data(), "E_COMPLETED_PASS") != nullptr;
        numPassed += passed ? 1 : 0;
        printf("--- result[%d] (test=%s) status=%d passed=%d ---\n%s\n", i, testIds[i], r, passed,
               buf.data());
    }

    AAXVal_Teardown();

    printf("AAXVAL_SUMMARY %d/%d passed\n", numPassed, numTests);
    return (numPassed == numTests) ? 0 : 1;
}
