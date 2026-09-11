# Real librime smoke

`run.ps1` links Mo's audited Rust wrapper to an already downloaded official
librime distribution and proves a real `nihao -> 你好` candidate and commit.
It performs no download. Pass `-Deploy` to compile the supplied shared Rime
data into the supplied user directory before running the smoke.

This is an opt-in integration gate; default workspace tests use a fake API
table so contributors do not silently link an arbitrary system librime.
