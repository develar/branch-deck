load("//defs:jvm.bzl", "jps_test", kt = "kotlin_library")
load(":local.bzl", "helper")

PACKAGE_VISIBILITY = ["//visibility:public"]

jps_test(
    name = "monolith",
    srcs = glob(["src/**/*.java"]),
    deps = [":runtime"],
    visibility = PACKAGE_VISIBILITY,
)

kt(
    name = "runtime",
    srcs = ["Runtime.kt"],
    testonly = True,
)

native.cc_library(
    name = "native_bridge",
    srcs = ["bridge.cc"],
)

helper(name = "")

exports_files(["README.md"])

def _tagged(name, tags = []):
    return tags + [name]

alias(
    name = _tagged("alias", ["manual"])[0],
    actual = ":runtime",
)
