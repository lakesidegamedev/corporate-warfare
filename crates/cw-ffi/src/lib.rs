//! Minimal packaging probe. The simulation session API is a later work packet.
uniffi::setup_scaffolding!();

#[derive(uniffi::Record)]
pub struct EngineInfo {
    pub version: String,
    pub interface_version: u32,
}

#[uniffi::export]
pub fn engine_info() -> EngineInfo {
    EngineInfo {
        version: cw_core::ENGINE_VERSION.to_owned(),
        interface_version: cw_core::INTERFACE_VERSION,
    }
}
