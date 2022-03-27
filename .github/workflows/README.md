### Fuzzing setup

ref-fvm uses [ClusterFuzzLite](https://google.github.io/clusterfuzzlite/) for contious fuzzing.
This consists of four workflows:
  - `cflite_pr.yml` - responsible for running fuzzing for PRs, it will only fuzz targets when diff affects files covered by given fuzzing target
  - `cflite_build.yml` - responsible for building fuzzing targets on master branch and storing them for recall. These recalled targets are used to detect if crasher is a new thing caused by changes or a newely discovered input which will also cause crash on master.
  - `cflite_batch.yml` - periodically (every 6h for 1h) runs fuzzing to develop the corpus which will be used for other on-demand fuzzing and as seed corpus for OSS-Fuzz.
  - `cflite_cron.yml` - every day generates coverage reports, as well as, prunes and mimizes the corpus.


Corpus and coverage data is stored within https://github.com/filecoin-project/ref-fvm-fuzz-corpora 


