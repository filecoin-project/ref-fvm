use std::env;
use std::fs::{self, File};
use std::io::{copy, BufWriter, Write};
use std::path::PathBuf;

use curl::easy::Easy;
use sha2::{Digest, Sha256};

const DEFAULT_FIL_BUILDIN_ACTORS_REPO_URL: &str =
    "https://github.com/filecoin-project/builtin-actors/releases/download";
const DEFAULT_FIL_BUILDIN_ACTORS_DOWNLOAD_ROOT: &str = "target";
const DEFAULT_FIL_BUILDIN_ACTORS_BUNDLE_PREFIX: &str = "bundle";
const DEFAULT_FIL_BUILDIN_ACTORS_ARTIFACT_PREFIX: &str = "builtin-actors";

// ============== release :: "dev/20220602" ==============
// cfg-01 | Failed
// const DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION: &str = "dev%2F20220602";
// const DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK: &str = "calibrationnet";

// cfg-02 | Failed
// const DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION: &str = "dev%2F20220602";
// const DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK: &str = "mainnet";

// cfg-03 | Failed
// const DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION: &str = "dev%2F20220602";
// const DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK: &str = "testing";

// cfg-04 | Failed
// const DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION: &str = "dev%2F20220602";
// const DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK: &str = "testing-fake-proofs";

// ============== release :: "dev%2F20220609-proofs" ==============
// cfg-01 | Failed
const DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION: &str = "dev%2F20220609-proofs";
const DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK: &str = "calibrationnet";

// cfg-02 | Failed
// const DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION: &str = "dev%2F20220609-proofs";
// const DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK: &str = "mainnet";

// cfg-03 |
// const DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION: &str = "dev%2F20220609-proofs";
// const DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK: &str = "testing";

// cfg-04 | Failed
// const DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION: &str = "dev%2F20220609-proofs";
// const DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK: &str = "testing-fake-proofs";

struct BuildinActorsArtifactsDownloader {
    fil_buildin_actors_repo_url: String,
    fil_buildin_actors_release_version: String,
    fil_buildin_actors_artifact_prefix: String,
    fil_buildin_actors_build_network: String,
    fil_buildin_actors_download_root: String,
}

impl Default for BuildinActorsArtifactsDownloader {
    fn default() -> Self {
        let fil_buildin_actors_repo_url = env::var("CONFIG_FIL_BUILDIN_ACTORS_REPO_URL")
            .unwrap_or_else(|_| String::from(DEFAULT_FIL_BUILDIN_ACTORS_REPO_URL));

        let fil_buildin_actors_release_version =
            env::var("CONFIG_FIL_BUILDIN_ACTORS_RELEASE_VERSION")
                .unwrap_or_else(|_| String::from(DEFAULT_FIL_BUILDIN_ACTORS_RELEASE_VERSION));

        let fil_buildin_actors_artifact_prefix =
            env::var("CONFIG_FIL_BUILDIN_ACTORS_ARTIFACT_PREFIX")
                .unwrap_or_else(|_| String::from(DEFAULT_FIL_BUILDIN_ACTORS_ARTIFACT_PREFIX));

        let fil_buildin_actors_build_network = env::var("CONFIG_FIL_BUILDIN_ACTORS_BUILD_NETWORK")
            .unwrap_or_else(|_| String::from(DEFAULT_FIL_BUILDIN_ACTORS_BUILD_NETWORK));

        let fil_buildin_actors_download_root = env::var("OUT_DIR")
            .unwrap_or_else(|_| String::from(DEFAULT_FIL_BUILDIN_ACTORS_DOWNLOAD_ROOT));

        Self {
            fil_buildin_actors_repo_url,
            fil_buildin_actors_release_version,
            fil_buildin_actors_artifact_prefix,
            fil_buildin_actors_build_network,
            fil_buildin_actors_download_root,
        }
    }
}

impl BuildinActorsArtifactsDownloader {
    fn download_artifacts(
        &self,
        source_url: &str,
        destination_path: &PathBuf,
    ) -> Result<Option<()>, String> {
        let mut easy = Easy::new();

        let dwnload_file_handle = File::create(destination_path).map_err(|err| -> String {
            format!(
                "Failed to create file {:#?} Err {:#?}",
                destination_path, err
            )
        })?;

        let mut writer = BufWriter::new(dwnload_file_handle);

        easy.follow_location(true).map_err(|err| -> String {
            format!("Curl Config follow_location failed Err {}", err)
        })?;

        easy.url(source_url)
            .map_err(|err| -> String { format!("Curl Config url failed Err {}", err) })?;

        easy.write_function(move |data| {
            Ok(writer.write(data).unwrap())
        })
        .map_err(|err| -> String {
            format!("Failed to download artifact {} Err {}", &source_url, err)
        })?;

        easy.perform().map_err(|err| -> String {
            format!(
                "Curl Failed to complete the download artifact {} Err {}",
                &source_url, err
            )
        })?;

        let response_code = easy.response_code().map_err(|err| -> String {
            format!(
                "Curl Failed to get response_code for {} Err {}",
                &source_url, err
            )
        })?;

        if response_code != 200 {
            return Err(format!(
                "Unexpected response code {} for {}",
                response_code, &source_url
            ));
        }

        Ok(Some(()))
    }

    fn verify_car_prebuild_image(&self) -> Result<Option<()>, String> {
        let artifact_download_url = format!(
            "{}/{}/{}-{}.{}",
            &self.fil_buildin_actors_repo_url,
            &self.fil_buildin_actors_release_version,
            &self.fil_buildin_actors_artifact_prefix,
            &self.fil_buildin_actors_build_network,
            "sha256",
        );

        let download_file_short = format!(
            "{}.{}",
            DEFAULT_FIL_BUILDIN_ACTORS_BUNDLE_PREFIX,
            "sha256",
        );

        let car_preimage_file_short = format!(
            "{}.{}",
            DEFAULT_FIL_BUILDIN_ACTORS_BUNDLE_PREFIX,
			"car",
        );

        let mut download_dir = PathBuf::from(&self.fil_buildin_actors_download_root);
		download_dir = download_dir
			.join(DEFAULT_FIL_BUILDIN_ACTORS_BUNDLE_PREFIX);

        // Absolute download file path ...
        let sha256_file_name = download_dir
            .join(download_file_short);

        // Absolute download file path ...
        let car_file_name = download_dir
            .join(car_preimage_file_short);

        assert!(
            car_file_name.exists(),
            "car file {:#?} not found",
            car_file_name
        );

        // download the sha256 release artifact file ...
        self.download_artifacts(&artifact_download_url, &sha256_file_name)?
            .unwrap();

        let mut car_file_handle = File::open(&car_file_name).map_err(|err| -> String {
            format!(
                "Failed to open car file {:#?} Err {:#?}",
                &car_file_name, err
            )
        })?;

        let mut hasher = Sha256::new();

        let _num_bytes = copy(&mut car_file_handle, &mut hasher)
            .map_err(|err| -> String { format!("IO Copy failed Err {}", err) })?;

        let hash = hasher.finalize();
        let compu_hex_hash = base16ct::lower::encode_string(&hash);

        let expected_hash_stream = &fs::read_to_string(&sha256_file_name)
            .map_err(|err| -> String { format!("FS Read failed Err {}", err) })?;

        let expected_hash_stream: Vec<&str> = expected_hash_stream.split(' ').collect();

        assert!(
            expected_hash_stream[0].eq(&compu_hex_hash),
            "Mismatch in SHA256 hash"
        );

        Ok(Some(()))
    }

    fn get_car_prebuild_image(&self) -> Result<Option<()>, String> {
        let artifact_download_url = format!(
            "{}/{}/{}-{}.{}",
            &self.fil_buildin_actors_repo_url,
            &self.fil_buildin_actors_release_version,
            &self.fil_buildin_actors_artifact_prefix,
            &self.fil_buildin_actors_build_network,
            "car",
        );

        let download_file_short = format!(
            "{}.{}",
            DEFAULT_FIL_BUILDIN_ACTORS_BUNDLE_PREFIX, "car",
        );

        let mut download_dir = PathBuf::from(&self.fil_buildin_actors_download_root);
		download_dir = download_dir
			.join(DEFAULT_FIL_BUILDIN_ACTORS_BUNDLE_PREFIX);

        if !download_dir.exists() {
            // safe to ignore the unit return code ...
            fs::create_dir_all(&download_dir).map_err(|err| -> String {
                format!(
                    "Failed to create download folder {:#?} Err {:#?}",
                    &download_dir.to_string_lossy(),
                    err,
                )
            })?;
        }

        // Absolute download file path ...
        let file_name = download_dir.join(download_file_short);

        if !file_name.exists() {
            // local file copy doesn't exist, trigger download ...
            self.download_artifacts(&artifact_download_url, &file_name)?
                .unwrap();

            self.verify_car_prebuild_image()?.unwrap();
        }

        Ok(Some(()))
    }
}

fn main() {
    let dwnld_runtime: BuildinActorsArtifactsDownloader = Default::default();

	// Trigger download, If no local copy, of the car prebuild artifacts.
    dwnld_runtime
        .get_car_prebuild_image()
        .map_err(|err| panic!("Artifact Download Failed | {:#?}", err))
        .unwrap();
}
