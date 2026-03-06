//! # ANK Prototype Protocol Definitions
//! This crate contains the compiled gRPC/Protobuf structures for the Aegis Neural Kernel.

/// Version 1 of the ANK Kernel Protocol.
pub mod v1 {
    tonic::include_proto!("ank.v1");

    pub mod siren {
        tonic::include_proto!("ank.v1.siren");
    }
}
