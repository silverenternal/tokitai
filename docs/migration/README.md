# Tokitai Migration Guides

This directory collects guides for upgrading between major and minor
releases of Tokitai.

## Available guides

| From     | To       | Guide                                    |
|----------|----------|------------------------------------------|
| `0.4.x`  | `0.5.0`  | [v0.4-to-v0.5.md](./v0.4-to-v0.5.md)     |

## Reading order

1. Start with the [TL;DR](./v0.4-to-v0.5.md#tldr) of the guide for the
   version you are upgrading to. Most readers stop here.
2. If the TL;DR mentions a non-trivial code change for your usage,
   jump to the matching "Migration scenarios" section.
3. The "Bug fixes that may surprise you" section lists fixes for known
   0.4 bugs — read this even if the TL;DR says your code is
   unaffected, because the macro may be generating different code now.

## Reporting a missing guide

If you are upgrading from a version that is not listed here, please
open an issue at <https://github.com/silverenternal/tokitai/issues>.
The repository only ships a guide when there is a documented
breaking or behaviour-changing difference between releases.
