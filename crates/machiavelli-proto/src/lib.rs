//! Wire-compatible subset of the installed Heraclitus Append API.
pub mod heraclitus {
    tonic::include_proto!("heraclitus.v1");
}

pub mod machiavelli {
    tonic::include_proto!("machiavelli.v1");
}
