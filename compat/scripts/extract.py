"""Regenerates the compat fixtures from checkouts of the downstream apps:

    python3 compat/scripts/extract.py path/to/studio path/to/wgpu-test

then `cargo fmt --all` (it leaves the `html!` bodies alone). Each
`//@ <file> <n>` line in a template is replaced by the n-th `html!`
block of that file, verbatim.
"""
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))


def blocks(path):
    source = open(path).read()
    found, start = [], 0
    while (begin := source.find("html! {", start)) >= 0:
        depth = 0
        for end in range(source.index("{", begin), len(source)):
            depth += {"{": 1, "}": -1}.get(source[end], 0)
            if depth == 0:
                break
        found.append(source[begin : end + 1])
        start = end + 1
    return found


def render(template, directory, out):
    text = open(os.path.join(here, template)).read()

    def block(match):
        return blocks(os.path.join(directory, match[1]))[int(match[2])]

    text = re.sub(r"//@ (\S+) (\d+)", block, text)
    with open(out, "w") as file:
        file.write(text)


studio, wgpu = sys.argv[1:3]
render("studio.rs.in", os.path.join(studio, "web-rust/src/ui"), os.path.join(here, "../studio/src/lib.rs"))
render("wgpu-test.rs.in", os.path.join(wgpu, "src/ui"), os.path.join(here, "../wgpu-test/src/lib.rs"))
