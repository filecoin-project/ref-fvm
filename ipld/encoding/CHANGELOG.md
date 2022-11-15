# Changelog

Changes to the FVM's shared encoding utilities.

## 0.3.0 [2022-10-11]

- Publicly use `serde` to expose it when developing actors.
- Expose a new `strict_bytes` module based on `serde_bytes`. This new module:
    - Refuses to decode anything that's not "bytes" (like `cs_serde_bytes`).
    - Can also decode into a fixed-sized array.
    - Has ~1% of the code of upstream.

## 0.2.2 [2022-06-13]

Change the hash length assert into an actual check, just in case.

## 0.2.1 [2022-05-19]

Update `serde_ipld_cbor` to 0.2.2.

## 0.2.0 [2022-04-29]

Update `serde_ipld_cbor` to 0.2.0, switching to cbor4ii.

The only breaking change is that `from_reader` now requires `io::BufRead`, not just `io::Read`.
