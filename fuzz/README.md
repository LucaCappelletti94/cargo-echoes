# Fuzz targets

ClusterFuzzLite runs these weekly in batch. Each target runs echoes on the 1 GiB stack `group` uses.

- `canonicalize` groups the input's source at two sites and expects one group.
- `schedules` compares legal typed schedules, primitive algebra, boolean rewrites, primitive value folds, literal folds, named-field ordering, literal alternatives, closed item order and inherent `impl` partitions with changed-neighbor controls, one family per input chosen by its last byte.
- `doc` writes NUL-separated parts as `///`, raw, escaped and `/** */` doc attributes and extracts them.
- `source` parses a whole file with `syn` and extracts it.

```sh
cargo +nightly fuzz run canonicalize fuzz/corpus/canonicalize fuzz/seeds/canonicalize -- -dict=fuzz/echoes.dict
```

## Seed Corpus

`fuzz/seeds/<target>/` holds one input per file, grown from echoes's own test strings and fixtures and reduced with `-set_cover_merge=1`. `.clusterfuzzlite/build.sh` zips it into the starting corpus and fails a target without seeds.
