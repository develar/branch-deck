load("@rules_java//java:defs.bzl", "java_library")

MAX_HEAP = "4g"
_PRIVATE_TAGS = ["manual"]
default_jvm_flags, extra_flags = ["-Xmx" + MAX_HEAP], []

def jps_test(name, srcs = [], deps = [], *extra, **kwargs):
    tags = _tagged(name)
    java_library(
        name = name,
        srcs = srcs,
        deps = deps + extra_flags,
        tags = tags,
        **kwargs
    )

def _tagged(name):
    for tag in _PRIVATE_TAGS:
        if tag in name:
            return [tag]
    return []

jps_test(name = "not_a_target_here")
