Symlink `./fuzz-corpora` to https://github.com/filecoin-project/ref-fvm-fuzz-corpora repo
to utilize existing corpus during fuzzing.

For example, assuming `ref-fvm-fuzz-corpora` is next to `ref-fvm`, run this in `testing`:

```shell
ln -s ../../ref-fvm-fuzz-corpora/ fuzz-corpora
```
