// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod calibration;
#[cfg(feature = "calibration")]
use calibration::*;
use fvm_gas_calibration_shared::*;

#[test]
#[cfg(feature = "calibration")]
fn on_block() {
    use std::collections::HashMap;

    use fvm::trace::ExecutionEvent;
    use fvm_shared::error::ExitCode;
    use rand::{thread_rng, Rng};

    let sizes = common_sizes();
    let iterations = 100;

    let mut all_obs: HashMap<String, Vec<Obs>> = Default::default();

    // NOTE: For actually modeling the effect of IO, we shouldn't be using the memory blockstore.
    // But at the same time when the contracts are executed the changes are buffered in mory,
    // not everything actually gets written to the disk.
    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    // NOTE: The order of sizes (doing them ascending, descending, or shuffled),
    // and whether we reuse the same tester or make a new one for each, does make a difference.

    for size in sizes.iter() {
        let params = OnBlockParams {
            size: *size,
            iterations,
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(Method::OnBlock as u64, &params);

        if let Some(failure) = ret.failure_info {
            panic!("message execution failed: {failure}");
        }
        assert_eq!(ret.msg_receipt.exit_code, ExitCode::OK);

        let mut iter_obs: HashMap<String, Vec<Obs>> = Default::default();

        for event in ret.exec_trace {
            if let ExecutionEvent::GasCharge(charge) = event {
                if charge.name.starts_with("OnBlock") {
                    if let Some(t) = charge.elapsed.get() {
                        let ob = Obs {
                            charge: charge.name.to_string(),
                            label: "n/a".into(),
                            elapsed_nanos: t.as_nanos(),
                            variables: vec![*size],
                            compute_gas: charge.compute_gas.as_milligas(),
                        };
                        iter_obs.entry(charge.name.into()).or_default().push(ob);
                    }
                }
            }
        }
        // The first OnBlockRead is for reading the parameters. From OnBlockStat that's the only record.
        iter_obs.get_mut("OnBlockRead").unwrap().remove(0);
        iter_obs.get_mut("OnBlockStat").unwrap().remove(0);

        for (name, mut obs) in iter_obs {
            if !obs.is_empty() {
                // According to the charts, there are odd outliers.
                obs = eliminate_outliers(obs, 0.02, Eliminate::Top);

                all_obs.entry(name).or_default().extend(obs);
            }
        }
    }

    for (name, obs) in all_obs {
        let regs = vec![least_squares("".into(), &obs, 0)];
        export(&name, &obs, &regs).unwrap();
    }
}

// TODO (fridrik): Enable this test after closing #1699
//#[test]
#[allow(dead_code)]
#[cfg(feature = "calibration")]
fn on_event_evm_shapes() {
    use fvm_shared::event::Flags;
    use rand::{thread_rng, Rng};

    const CHARGE_VALIDATE: &str = "OnActorEventValidate";
    const CHARGE_ACCEPT: &str = "OnActorEventAccept";
    const METHOD: Method = Method::OnEvent;

    let entries = 1..=5;
    let (key_size, value_size) = (2, 32); // 2 bytes per key, 32 bytes per value (topics)
    let last_entry_value_sizes = (5u32..=13).map(|n| u64::pow(2, n) as usize); // 32 bytes to 8KiB (payload)

    let iterations = 500;

    let (mut validate_obs, mut accept_obs) = (Vec::new(), Vec::new());

    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    for entry_count in entries {
        for last_entry_value_size in last_entry_value_sizes.clone() {
            let label = format!("{entry_count:?}entries");
            let params = OnEventParams {
                iterations,
                // number of entries to emit
                entries: entry_count,
                mode: EventCalibrationMode::Shape((key_size, value_size, last_entry_value_size)),
                flags: Flags::FLAG_INDEXED_ALL,
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            // Estimated length of the CBOR payload (confirmed with observations)
            // 1 is the list header; 5 per entry CBOR overhead + flags.
            let len = 1
                + ((entry_count - 1) * value_size)
                + last_entry_value_size
                + entry_count * key_size
                + entry_count * 5;

            {
                let mut series = collect_obs(&ret.clone(), CHARGE_VALIDATE, &label, len);
                series = eliminate_outliers(series, 0.02, Eliminate::Top);
                validate_obs.extend(series);
            };

            {
                let mut series = collect_obs(&ret.clone(), CHARGE_ACCEPT, &label, len);
                series = eliminate_outliers(series, 0.02, Eliminate::Top);
                accept_obs.extend(series);
            };
        }
    }

    for (obs, name) in vec![(validate_obs, CHARGE_VALIDATE), (accept_obs, CHARGE_ACCEPT)].iter() {
        let regression = run_linear_regression(obs);

        export(name, obs, &regression).unwrap();
    }
}

// intentionally left disabled since we're not interested in these observations at this stage.
#[allow(dead_code)]
fn on_event_target_size() {
    const CHARGE_VALIDATE: &str = "OnActorEventValidate";
    const CHARGE_ACCEPT: &str = "OnActorEventAccept";
    const METHOD: Method = Method::OnEvent;

    use calibration::*;
    use fvm_shared::event::Flags;
    use rand::{thread_rng, Rng};

    let mut config: Vec<(usize, usize)> = vec![];
    // 1 entry, ranging 8..1024 bytes
    config.extend((3u32..=10).map(|n| (1usize, u64::pow(2, n) as usize)));
    // 2 entry, ranging 16..1024 bytes
    config.extend((4u32..=10).map(|n| (2usize, u64::pow(2, n) as usize)));
    // 4 entries, ranging 32..1024 bytes
    config.extend((5u32..=10).map(|n| (4usize, u64::pow(2, n) as usize)));
    // 8 entries, ranging 64..1024 bytes
    config.extend((6u32..=10).map(|n| (8usize, u64::pow(2, n) as usize)));
    // 16 entries, ranging 128..1024 bytes
    config.extend((7u32..=10).map(|n| (16usize, u64::pow(2, n) as usize)));
    // 32 entries, ranging 256..1024 bytes
    config.extend((8u32..=10).map(|n| (32usize, u64::pow(2, n) as usize)));
    // 64 entries, ranging 512..1024 bytes
    config.extend((9u32..=10).map(|n| (64usize, u64::pow(2, n) as usize)));

    let iterations = 500;

    let (mut validate_obs, mut accept_obs) = (Vec::new(), Vec::new());

    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    for (entries, target_size) in config.iter() {
        let label = format!("{entries:?}entries");
        let params = OnEventParams {
            iterations,
            // number of entries to emit
            entries: *entries,
            // target size of the encoded CBOR; this is approximate.
            mode: EventCalibrationMode::TargetSize(*target_size),
            flags: Flags::FLAG_INDEXED_ALL,
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(METHOD as u64, &params);

        {
            let mut series = collect_obs(&ret.clone(), CHARGE_VALIDATE, &label, *target_size);
            series = eliminate_outliers(series, 0.02, Eliminate::Top);
            validate_obs.extend(series);
        };

        {
            let mut series = collect_obs(&ret.clone(), CHARGE_ACCEPT, &label, *target_size);
            series = eliminate_outliers(series, 0.02, Eliminate::Top);
            accept_obs.extend(series);
        };
    }

    for (obs, name) in vec![(validate_obs, CHARGE_VALIDATE), (accept_obs, CHARGE_ACCEPT)].iter() {
        let regression = run_linear_regression(obs);

        export(name, obs, &regression).unwrap();
    }
}

#[test]
#[cfg(feature = "calibration")]
fn on_hashing() {
    use fvm_shared::crypto::hash::SupportedHashes;
    use rand::{thread_rng, Rng};

    const CHARGE_NAME: &str = "OnHashing";
    const METHOD: Method = Method::OnHashing;

    let hashers = vec![
        SupportedHashes::Sha2_256,
        SupportedHashes::Blake2b256,
        SupportedHashes::Blake2b512,
        SupportedHashes::Keccak256,
        SupportedHashes::Ripemd160,
    ];

    let sizes = common_sizes();
    let iterations = 100;

    let mut te = instantiate_tester();
    let mut obs = Vec::new();
    let mut rng = thread_rng();

    for hasher in hashers.iter() {
        let label = format!("{hasher:?}");
        for size in sizes.iter() {
            let params = OnHashingParams {
                hasher: *hasher as u64,
                size: *size,
                iterations,
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let iter_obs = collect_obs(&ret, CHARGE_NAME, &label, *size);

            // According to the charts there is always an outlier with 10x runtime,
            // which can throw off the model. Maybe it's while some things are warming up.
            // Seems to be present at each call, so once per size. I'll just throw these away.
            let iter_obs = eliminate_outliers(iter_obs, 0.02, Eliminate::Top);

            obs.extend(iter_obs);
        }
    }

    let regression = run_linear_regression(&obs);

    export(CHARGE_NAME, &obs, &regression).unwrap();
}

#[test]
#[cfg(feature = "calibration")]
fn on_recover_secp_public_key() {
    use rand::{thread_rng, Rng, RngCore};

    const CHARGE_NAME: &str = "OnRecoverSecpPublicKey";
    const METHOD: Method = Method::OnRecoverSecpPublicKey;

    // Just doing it for uniformity.
    let sizes = common_sizes();
    let iterations = 10;

    let mut te = instantiate_tester();
    let mut obs = Vec::new();
    let mut rng = thread_rng();

    // Generate a signature over some data to ensure it's not complete rubbish.
    let mut data = vec![0u8; 100];
    rng.fill_bytes(&mut data);

    let sk = libsecp256k1::SecretKey::random(&mut rng);
    let sig = secp_sign(&sk, &data);

    for size in sizes.iter() {
        let params = OnRecoverSecpPublicKeyParams {
            iterations,
            size: *size,
            signature: sig.to_vec(),
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(METHOD as u64, &params);

        let iter_obs = collect_obs(&ret, CHARGE_NAME, "n/a", *size);
        //let iter_obs = eliminate_outliers(iter_obs, 0.02, Eliminate::Top);

        obs.extend(iter_obs);
    }

    let regs = vec![least_squares("".into(), &obs, 0)];

    export(CHARGE_NAME, &obs, &regs).unwrap();
}

#[test]
#[cfg(feature = "calibration")]
fn on_send() {
    const TRANSFER_CHARGE_NAME: &str = "OnValueTransfer";
    const INVOKE_CHARGE_NAME: &str = "OnMethodInvocation";
    const METHOD: Method = Method::OnSend;

    let iterations = 100;

    let mut te = instantiate_tester();
    let mut invoke_obs = Vec::new();
    let mut transfer_obs = Vec::new();

    for invoke in [true, false] {
        for value_transfer in [true, false] {
            let label = match (invoke, value_transfer) {
                (true, true) => "invoke-and-transfer",
                (false, true) => "transfer-only",
                (true, false) => "invoke-only",
                (false, false) => continue,
            };
            let params = OnSendParams {
                iterations,
                value_transfer,
                invoke,
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let both = (value_transfer == invoke) as usize;

            if value_transfer {
                let iter_transfer_obs = collect_obs(&ret, TRANSFER_CHARGE_NAME, label, both);
                let iter_transfer_obs = eliminate_outliers(iter_transfer_obs, 0.02, Eliminate::Top);
                transfer_obs.extend(iter_transfer_obs);
            }

            if invoke {
                let iter_invoke_obs = collect_obs(&ret, INVOKE_CHARGE_NAME, label, both);
                let iter_invoke_obs = eliminate_outliers(iter_invoke_obs, 0.02, Eliminate::Top);
                invoke_obs.extend(iter_invoke_obs);
            }
        }
    }

    let transfer_regs = vec![least_squares("".into(), &transfer_obs, 0)];
    export(TRANSFER_CHARGE_NAME, &transfer_obs, &transfer_regs).unwrap();
    let invoke_regs = vec![least_squares("".into(), &invoke_obs, 0)];
    export(INVOKE_CHARGE_NAME, &invoke_obs, &invoke_regs).unwrap();
}

#[test]
#[cfg(feature = "calibration")]
fn on_verify_signature() {
    use bls_signatures::Serialize;
    use fvm_shared::address::Address;
    use fvm_shared::crypto::signature::SignatureType;
    use rand::{thread_rng, Rng, RngCore};

    const CHARGE_NAME: &str = "OnVerifySignature";
    const METHOD: Method = Method::OnVerifySignature;

    let sig_types = vec![SignatureType::BLS, SignatureType::Secp256k1];

    let sizes = common_sizes();
    let iterations = 100;

    let mut te = instantiate_tester();
    let mut obs = Vec::new();
    let mut rng = thread_rng();

    // Just some random data over which we can generate an example signature.
    // Having a valid BLS signature is important otherwise verification is
    // an instant rejection without hasing the input data.
    let mut data = vec![0u8; 100];
    rng.fill_bytes(&mut data);

    for sig_type in sig_types.iter() {
        let label = format!("{sig_type:?}");

        let (signer, signature) = match sig_type {
            SignatureType::Secp256k1 => {
                let sk = libsecp256k1::SecretKey::random(&mut rng);
                let pk = libsecp256k1::PublicKey::from_secret_key(&sk);
                let addr = Address::new_secp256k1(&pk.serialize()).unwrap();
                let sig = secp_sign(&sk, &data).into();
                (addr, sig)
            }
            SignatureType::BLS => {
                let sk = bls_signatures::PrivateKey::generate(&mut rng);
                let pk = sk.public_key();
                let addr = Address::new_bls(&pk.as_bytes()).unwrap();
                let sig = sk.sign(&data).as_bytes();
                (addr, sig)
            }
        };

        for size in sizes.iter() {
            let params = OnVerifySignatureParams {
                iterations,
                size: *size,
                signer,
                signature: signature.clone(),
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let iter_obs = collect_obs(&ret, CHARGE_NAME, &label, *size);
            let iter_obs = eliminate_outliers(iter_obs, 0.02, Eliminate::Top);

            obs.extend(iter_obs);
        }
    }

    let regression = run_linear_regression(&obs);

    export(CHARGE_NAME, &obs, &regression).unwrap();
}
