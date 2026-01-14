pub enum Endpoint {
    Sgv,
    Mbg,
    Iob,
    Entries,
    Treatments,
    Properties,
    Current,
    DeviceStatus,
    Profile,
}

impl Endpoint {
    pub fn as_path(&self) -> &'static str {
        match self {
            Endpoint::Entries => "api/v2/entries.json",
            Endpoint::Current => "api/v2/entries/current.json",
            Endpoint::Sgv => "api/v2/entries/sgv.json",
            Endpoint::Mbg => "api/v2/entries/mbg.json",
            Endpoint::Iob => "api/v2/properties/iob.json",
            Endpoint::Treatments => "api/v2/treatments.json",
            Endpoint::Properties => "api/v2/properties.json",
            Endpoint::DeviceStatus => "api/v2/devicestatus.json",
            Endpoint::Profile => "api/v2/profile.json",
        }
    }
}
