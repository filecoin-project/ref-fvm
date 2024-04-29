// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod calibration;
#[cfg(feature = "calibration")]
use calibration::*;
#[cfg(feature = "calibration")]
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

#[test]
#[cfg(feature = "calibration")]
fn on_event_by_value_size() {
    use fvm_shared::event::Flags;
    use rand::{thread_rng, Rng};

    const CHARGE: &str = "OnActorEvent";
    const METHOD: Method = Method::OnEvent;

    let iterations = 500;
    let mut te = instantiate_tester();
    let mut rng = thread_rng();

    let mut obs = Vec::new();

    let entry_counts = &[1usize, 16, 127, 255];
    for &entries in entry_counts {
        for total_value_size in (8..=13).map(|x| usize::pow(2, x)) {
            let label = format!("{entries}-entries");
            let params = OnEventParams {
                iterations,
                // number of entries to emit
                entries,
                total_value_size,
                flags: Flags::FLAG_INDEXED_ALL,
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let mut series = collect_obs(&ret.clone(), CHARGE, &label, total_value_size);
            series = eliminate_outliers(series, 0.02, Eliminate::Top);
            obs.extend(series);
        }
    }

    let regression = run_linear_regression(&obs);

    export("OnActorEventValue", &obs, &regression).unwrap();
}

#[test]
#[cfg(feature = "calibration")]
fn on_event_by_entry_count() {
    use fvm_shared::event::Flags;
    use rand::{thread_rng, Rng};

    const CHARGE: &str = "OnActorEvent";
    const METHOD: Method = Method::OnEvent;

    let iterations = 500;
    let mut te = instantiate_tester();
    let mut rng = thread_rng();

    let mut obs = Vec::new();

    let total_value_sizes = &[255, 1024, 4096, 8192];
    for &total_value_size in total_value_sizes {
        for entries in (1..=8).map(|x| usize::pow(2, x) - 1) {
            let label = format!("{total_value_size}-size");
            let params = OnEventParams {
                iterations,
                // number of entries to emit
                entries,
                total_value_size,
                flags: Flags::FLAG_INDEXED_ALL,
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let mut series = collect_obs(&ret.clone(), CHARGE, &label, entries);
            series = eliminate_outliers(series, 0.02, Eliminate::Top);
            obs.extend(series);
        }
    }

    let regression = run_linear_regression(&obs);

    export("OnActorEventEntries", &obs, &regression).unwrap();
}

#[test]
#[cfg(feature = "calibration")]
fn utf8_validation() {
    use fvm::gas::price_list_by_network_version;
    use fvm_shared::version::NetworkVersion;
    use rand::{distributions::Standard, thread_rng, Rng};

    let mut chars = thread_rng().sample_iter(Standard);
    const CHARGE: &str = "OnUtf8Validate";

    let iterations = 500;
    let price_list = price_list_by_network_version(NetworkVersion::V21);

    let mut obs = Vec::new();
    #[derive(Debug, Copy, Clone)]
    enum Kind {
        Ascii,
        MaxUtf8,
        RandomUtf8,
    }
    use Kind::*;
    for size in (0..=8).map(|x| usize::pow(2, x)) {
        for kind in [Ascii, RandomUtf8, MaxUtf8] {
            let mut series = Vec::new();
            for _ in 0..iterations {
                let rand_str: String = match kind {
                    Ascii => "a".repeat(size),
                    MaxUtf8 => char::REPLACEMENT_CHARACTER.to_string().repeat(size / 2),
                    RandomUtf8 => chars
                        .by_ref()
                        .take_while({
                            let mut total: usize = 0;
                            move |c: &char| {
                                total += c.len_utf8();
                                total < size
                            }
                        })
                        .collect(),
                };
                let charge = price_list.on_utf8_validation(rand_str.len());
                let start = minstant::Instant::now();
                let _ = std::hint::black_box(std::str::from_utf8(std::hint::black_box(
                    rand_str.as_bytes(),
                )));
                let time = start.elapsed();
                series.push(Obs {
                    charge: CHARGE.into(),
                    label: format!("{:?}-validate", kind),
                    elapsed_nanos: time.as_nanos(),
                    variables: vec![rand_str.len()],
                    compute_gas: charge.compute_gas.as_milligas(),
                })
            }
            obs.extend(eliminate_outliers(series, 0.02, Eliminate::Both));
        }
    }

    let regression = run_linear_regression(&obs);

    export(CHARGE, &obs, &regression).unwrap();
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

#[test]
#[cfg(feature = "calibration")]
fn on_verify_bls_aggregate() {
    use bls_signatures::Serialize;
    use rand::{thread_rng, RngCore};

    const CHARGE_NAME: &str = "OnVerifyBlsAggregateSignature";
    const METHOD: Method = Method::OnVerifyBlsAggregate;

    let iterations = 100;

    let mut te = instantiate_tester();
    let mut obs = Vec::new();
    let mut rng = thread_rng();

    for &n in &[1, 4, 8, 20, 50, 200, 1000] {
        let mut keys = Vec::new();
        let mut sks = Vec::new();
        let mut sigs = Vec::new();
        let mut messages = Vec::new();
        for _ in 0..n {
            let mut data = vec![0u8; 100];
            rng.fill_bytes(&mut data);
            let sk = bls_signatures::PrivateKey::generate(&mut rng);
            let pk = sk.public_key();
            let sig = sk.sign(&data);

            keys.push(pk.as_bytes());
            sks.push(sk);
            messages.push(data);
            sigs.push(sig);
        }
        let signature = bls_signatures::aggregate(&sigs).unwrap().as_bytes();
        let params = OnVerifyBlsAggregateParams {
            iterations,
            signature,
            keys,
            messages,
        };

        let ret = te.execute_or_die(METHOD as u64, &params);

        let iter_obs = collect_obs(&ret, CHARGE_NAME, "signers", n);
        let iter_obs = eliminate_outliers(iter_obs, 0.02, Eliminate::Top);

        obs.extend(iter_obs);
    }

    let regression = run_linear_regression(&obs);

    export(CHARGE_NAME, &obs, &regression).unwrap();
}

// Scan CBOR Fields with no links.
#[test]
#[cfg(feature = "calibration")]
fn on_scan_cbor_fields() {
    use std::collections::HashMap;

    use fvm::trace::ExecutionEvent;
    use fvm_shared::error::ExitCode;
    use rand::{thread_rng, Rng};

    let field_counts = [2, 5, 10, 50, 100, 1000, 2500, 5000, 7500, 10_000];
    let iterations = 500;

    let mut all_obs: HashMap<String, Vec<Obs>> = Default::default();
    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    for fc in field_counts.iter().copied() {
        let params = OnScanIpldLinksParams {
            cbor_link_count: 0,
            cbor_field_count: fc,
            iterations,
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(Method::OnScanIpldLinks as u64, &params);

        if let Some(failure) = ret.failure_info {
            panic!("message execution failed: {failure}");
        }
        assert_eq!(ret.msg_receipt.exit_code, ExitCode::OK);

        let mut iter_obs: HashMap<String, Vec<Obs>> = Default::default();

        for event in ret.exec_trace {
            if let ExecutionEvent::GasCharge(charge) = event {
                if charge.name.starts_with("OnScanIpldLinks") {
                    if let Some(t) = charge.elapsed.get() {
                        let ob = Obs {
                            charge: charge.name.into(),
                            label: "n/a".into(),
                            elapsed_nanos: t.as_nanos(),
                            variables: vec![fc],
                            compute_gas: charge.compute_gas.as_milligas(),
                        };
                        iter_obs
                            .entry("OnScanCborFields".into())
                            .or_default()
                            .push(ob);
                    }
                }
            }
        }

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

// Scan CBOR Links, keeping the fields constant (10,000).
#[test]
#[cfg(feature = "calibration")]
fn on_scan_cbor_links() {
    use std::collections::HashMap;

    use fvm::trace::ExecutionEvent;
    use fvm_shared::error::ExitCode;
    use rand::{thread_rng, Rng};

    let field_count = 10_000;
    let link_counts = [1, 10, 20, 50, 100, 500, 1000, 2500];
    let iterations = 250;

    let mut all_obs: HashMap<String, Vec<Obs>> = Default::default();
    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    for lc in link_counts.iter().copied() {
        let params = OnScanIpldLinksParams {
            cbor_link_count: lc,
            cbor_field_count: field_count,
            iterations,
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(Method::OnScanIpldLinks as u64, &params);

        if let Some(failure) = ret.failure_info {
            panic!("message execution failed: {failure}");
        }
        assert_eq!(ret.msg_receipt.exit_code, ExitCode::OK);

        let mut iter_obs: HashMap<String, Vec<Obs>> = Default::default();

        for event in ret.exec_trace {
            let ExecutionEvent::GasCharge(charge) = event else {
                continue;
            };
            for (key, name) in [
                ("OnScanIpldLinks", "OnScanIpldLinks"),
                ("OnTrackLinks", "OnBlockOpen"),
                ("OnCheckLinks", "OnBlockCreate"),
            ] {
                if charge.name != name {
                    continue;
                }
                let Some(t) = charge.elapsed.get() else {
                    continue;
                };

                let ob = Obs {
                    charge: charge.name.into(),
                    label: "n/a".into(),
                    elapsed_nanos: t.as_nanos(),
                    variables: vec![lc],
                    compute_gas: charge.compute_gas.as_milligas(),
                };
                iter_obs.entry(key.into()).or_default().push(ob);
                break;
            }
        }

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
