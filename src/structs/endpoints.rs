pub enum Endpoint {
    Svg,
    Mbg,
    Iob,
    Entries,
}

impl Endpoint {
    pub fn as_path(&self) -> &'static str {
        match self {
            Endpoint::Entries => "api/v2/entries.json",
            Endpoint::Svg => "api/v2/entries/sgv.json",
            Endpoint::Mbg => "api/v2/entries/mbg.json",
            Endpoint::Iob => "api/v2/properties/iob.json",
        }
    }
}
