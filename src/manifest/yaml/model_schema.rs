use super::model::Manifest;

pub fn schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::default()
        .into_generator()
        .into_root_schema_for::<Manifest>()
}
