#![no_main]

use libfuzzer_sys::fuzz_target;
use std::borrow::Cow;
use thalassa::{TlsplDeserialize, TlsplSerialize, TlsplSize};

fuzz_target!(|data: &[u8]| {
    let Ok(serialized) = data.tlspl_serialize() else {
        return;
    };

    assert_eq!(data.tlspl_serialized_len(), serialized.len());

    let Ok(deserialized) = Cow::<[u8]>::tlspl_deserialize_from(&mut serialized.as_slice()) else {
        return;
    };

    assert_eq!(data, &*deserialized);
});
